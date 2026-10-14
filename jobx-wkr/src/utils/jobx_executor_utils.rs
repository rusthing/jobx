use crate::config::JobxExecutorConfig;
use anyhow::Result;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use jobx_api::dic::{JobType, TaskStatus, TaskType};
use jobx_api::vo::{JobxJobVo, JobxTaskVo};
use jobx_api_client::api_client::{JobxApiClient, get_jobx_api_client};
use jobx_api_client::dto::{JobxTaskAddDto, JobxTaskModifyDto};
use redis::Value as RedisValue;
use redis::streams::StreamId;
use robotech::api::{U32, U64};
use robotech::cfg::CfgError;
use robotech::env::get_app_instance_id;
use robotech::redis::{
    ack_stream_message, auto_claim_stale_messages, delete_group_consumer, ensure_consumer_group,
    list_group_consumers, read_from_stream,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, trace, warn};
use wheel_rs::time_utils::{build_ticker, now_ms};

static JOBX_EXECUTOR_CONFIGS: ArcSwapOption<HashMap<String, Arc<JobxExecutorConfig>>> =
    ArcSwapOption::const_empty();

/// 消费者组的起始消费位点（建组那一刻生效）。
///
/// 这里用 `"0"` 而不是 `"$"`：`jobx_task` 的调度推进（`next_assign_ms`）只在
/// worker 成功 `take` 之后才发生，因此「流里已发布但从未投递过的消息」= 「这次任务
/// 永远不会执行」。建组时若用 `"$"`，这些消息会被直接跳过，且因为 `list_publishable`
/// 的 NOT EXISTS 幂等条件（依赖 jobx_task 行）始终成立，调度器会一直重发、worker 一直
/// 收不到，形成死循环。用 `"0"` 让流中现存消息全部被投递，重复投递由 `jobx_task`
/// 的 (job_id, scheduled_exec_start_ms) 唯一约束兜底。
const CONSUMER_GROUP_LAST_ID: &str = "0";

pub fn get_jobx_executor_config(executor_code: &str) -> Result<Arc<JobxExecutorConfig>, CfgError> {
    let configs = JOBX_EXECUTOR_CONFIGS
        .load_full()
        .ok_or_else(|| CfgError::NotInit("Executor config not initialized".to_string()))?;
    configs
        .get(executor_code)
        .cloned()
        .ok_or_else(|| CfgError::NotInit(format!("Executor config not found for: {executor_code}")))
}

pub fn set_jobx_executor_config(config: JobxExecutorConfig) {
    let code = config.executor_code.clone();
    JOBX_EXECUTOR_CONFIGS.rcu(|current| {
        let mut new_configs = match current {
            Some(existing) => (**existing).clone(),
            None => HashMap::new(),
        };
        new_configs.insert(code.clone(), Arc::new(config.clone()));
        Some(Arc::new(new_configs))
    });
}

/// 从 `JobxJobVo` 构建 `JobxTaskAddDto`
///
/// 将任务计划中的通用属性（`executor_code`、`exec_params`、`job_type`、`cron`、
/// `interval_duration` 等）映射到任务记录 DTO 中。
///
/// 若 `job.next_assign_ms` 不为空表示这是计划执行任务（`TaskType::Scheduled`），
/// 否则为立即执行任务（`TaskType::Immediate`）。
///
/// 此函数在 executor 收到 Redis Stream 消息后被调用，构建出 DTO 后通过 API client
/// 的 `take` 接口提交给服务端创建任务记录。集中在此处构建可避免在多处重复手写字段赋值。
fn build_task_add_dto(job: &JobxJobVo, instance_id: String, user_id: U64) -> JobxTaskAddDto {
    let task_type = if job.next_assign_ms.is_some() {
        TaskType::Scheduled
    } else {
        TaskType::Immediate
    };

    JobxTaskAddDto::builder()
        .task_type(task_type)
        .job_id(Some(job.id.into()))
        .assign_ms(U64::from(now_ms()))
        .scheduled_exec_start_ms(
            job.next_assign_ms
                .zip(job.assign_lead_ms)
                .map(|(next_ms, lead_ms)| U64::from(u64::from(next_ms) + u64::from(lead_ms))),
        )
        .executor_code(job.executor_code.clone())
        .executor_instance(Some(instance_id))
        .exec_params(job.params.clone())
        .job_type(job.job_type)
        .high_freq(job.high_freq)
        .cron(job.cron.clone())
        .interval_duration(job.interval_duration)
        .valid_begin_ms(job.valid_begin_ms.map(|v| v.into()))
        .valid_end_ms(job.valid_end_ms.map(|v| v.into()))
        .assign_lead_ms(job.assign_lead_ms.map(U32::from))
        ._current_user_id(user_id)
        .build()
}

/// # 消息处理 trait
///
/// Executor 从 Redis Stream 收到任务消息后，自动创建任务记录并反序列化为 `JobxTaskVo`，
/// 并调用此 trait 进行处理。
/// 下游客户端需实现此 trait 来完成实际的任务执行逻辑。
///
/// 返回 `Ok(String)` 表示执行成功，`Err(anyhow::Error)` 表示执行失败，
/// 错误信息会被记录到日志和任务执行结果中。
///
/// ## 使用示例
///
/// ```no_run
/// use async_trait::async_trait;
/// use jobx_api::vo::JobxTaskVo;
/// use jobx_wkr::JobxTaskHandler;
///
/// struct MyHandler;
///
/// #[async_trait]
/// impl JobxTaskHandler for MyHandler {
///     async fn handle(&self, task: &JobxTaskVo) -> Result<String> {
///         // task 包含任务的所有信息：executor_code、exec_params、job_type、task_id 等
///         // 下游客户端根据 task 信息执行具体业务逻辑
///         // 返回 Err 表示任务执行失败，错误信息将被记录
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait JobxTaskHandler: Send + Sync + 'static {
    /// 处理任务
    /// - task: 任务视图对象，包含 executor_code、exec_params、job_type、task_id 等完整字段
    /// - 返回 `Ok(())` 表示执行成功，`Err` 表示执行失败
    async fn handle(&self, task: &JobxTaskVo) -> Result<String>;
}

pub async fn init_jobx_executor(
    executor_config: JobxExecutorConfig,
    handler: Arc<dyn JobxTaskHandler>,
) -> Result<(), CfgError> {
    info!("setup jobx executor config...: {executor_config:?}");
    set_jobx_executor_config(executor_config.clone());
    let app_instance_id = get_app_instance_id();
    if let Some(executor_group) = &executor_config.executor_group {
        let stream_key = build_stream_key(
            &executor_config.executor_key,
            &executor_config.executor_code,
        );
        ensure_consumer_group(&stream_key, executor_group, CONSUMER_GROUP_LAST_ID)
            .await
            .map_err(|e| CfgError::Init(e.to_string()))?;
    }
    // 启动扫描线程
    tokio::spawn(run_executor_loop(
        app_instance_id.to_string(),
        executor_config.executor_code.clone(),
        executor_config.scan_interval,
        handler,
    ));
    Ok(())
}

/// # 启动工作者订阅循环
///
/// 按配置的扫描间隔，以消费者组模式从 Redis Stream 拉取任务消息。
/// 每轮循环通过 `XREADGROUP ... ">"` 接收**从未投递给组内任何消费者**的新消息，
/// 收到后逐一创建任务记录（take）并交给 `handler` 执行业务逻辑。
///
/// `">"` 只返回新消息，不会重复投递已在组内分配过的消息；
/// 如果需要处理崩溃恢复场景（认领其他消费者未 ACK 的 pending 消息），
/// 需要额外用 `"0"` 读取 PEL。
///
/// 消费者组自愈：`init_jobx_executor` 只在启动时建一次组，若运行期间组被删掉
/// （Redis 被清空、key 过期、`XGROUP DESTROY`），此后每轮读取都会报 `NOGROUP`。
/// 循环检测到该错误会就地调用 `ensure_consumer_group` 幂等重建，无需重启进程。
///
/// 运行期间会动态读取最新配置，若扫描间隔变更则自动调整 ticker。
/// 该函数不会返回，需通过 `tokio::spawn` 在独立任务中运行。
///
/// 除拉新消息外，还按 `maintenance_interval` 低频执行消费者组维护
/// （见 [`run_group_maintenance`]）。
async fn run_executor_loop(
    app_instance_id: String,
    executor_code: String,
    initial_scan_interval: Duration,
    handler: Arc<dyn JobxTaskHandler>,
) {
    let mut ticker = build_ticker(initial_scan_interval);
    let mut last_maintenance = Instant::now();
    loop {
        ticker.tick().await;
        let executor_config = match get_jobx_executor_config(&executor_code) {
            Ok(c) => c,
            Err(e) => {
                warn!("获取工作者配置失败: {e:?}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        let stream_key = build_stream_key(
            &executor_config.executor_key,
            &executor_config.executor_code,
        );
        let group = executor_config
            .executor_group
            .as_ref()
            .map(|g| (g.as_str(), app_instance_id.as_str()));
        // 不传 block_duration（= 非阻塞轮询），靠 scan_interval 控制节奏。
        //
        // 这里刻意不用 `XREAD ... BLOCK`：redis-rs 从 1.0 起给 async 连接加了默认
        // `response_timeout = 500ms`（见 `redis-1.7.1/src/client.rs` 的
        // DEFAULT_RESPONSE_TIMEOUT，`get_multiplexed_async_connection()` 即取该默认值），
        // 而 `get_redis_conn()` 返回的是全进程共享的一条 MultiplexedConnection。
        // 于是 `BLOCK 5000` 会在 500ms 就被客户端掐断返回 `Err(timed out)`；
        // 更糟的是客户端放弃后**不会取消服务端已发出的 XREADGROUP** —— 消息到达时服务端
        // 照常投递并写进消费者 PEL，回复却被丢弃，消息就此永久滞留 PEL（`">"` 不再返回它），
        // 且 `is_timeout()` 分支只打 trace!，在 info 级别下完全静默。
        // 要安全地用 BLOCK，必须给它独占连接并把 response_timeout 调大或关掉，代价大于收益。
        // ">" = XREADGROUP 模式下仅接收未投递过的新消息，不会重复拿到已分配的消息
        // 末位 noack：配置为 true 时走 `NOACK`，消息不进 PEL，也就没有后续 XACK。
        let reply = match read_from_stream(
            &stream_key,
            ">",
            executor_config.max_messages,
            &None,
            group,
            executor_config.noack,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    trace!("读取 Redis Stream 超时: {e:?}");
                } else if e.to_string().contains("NOGROUP") {
                    // 消费者组（或流本身）不存在。Redis 被清空、key 过期、组被 XGROUP DESTROY
                    // 等都会让每轮读取都报 NOGROUP，此时不能只记日志空转 —— 就地幂等重建，
                    // 让循环无需重启进程即可自愈。
                    warn!("消费者组不存在，就地重建: {e:?}");
                    if let Some(group_name) = executor_config.executor_group.as_deref() {
                        match ensure_consumer_group(&stream_key, group_name, CONSUMER_GROUP_LAST_ID)
                            .await
                        {
                            Ok(()) => {
                                info!("消费者组重建成功: stream={stream_key}, group={group_name}")
                            }
                            Err(re) => warn!("消费者组重建失败: {re:?}"),
                        }
                    }
                } else {
                    warn!("读取 Redis Stream 失败: {e:?}");
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        // 遍历 Redis Stream 回复中的每一条 Stream Key。
        // 一次 XREAD[GROUP] 请求可以同时监听多个 Stream，
        // 返回结果按 Stream Key 分组，每个 Stream Key 可能包含若干条消息。
        for stream_key in reply.keys {
            // 当前 Stream Key 下的每条消息对应一个待领取的任务计划（Job）。
            // 分发策略（逐条处理 / 只处理最后一条）见 [`dispatch_stream_messages`]。
            dispatch_stream_messages(
                &stream_key.key,
                stream_key.ids,
                &app_instance_id,
                executor_config.executor_group.as_deref(),
                &executor_config,
                &handler,
            )
            .await;
        }

        // ---- 消费者组维护（低频，与拉新消息解耦）----
        // 维护只对消费者组模式有意义（无组时既没有 PEL 也没有消费者条目）。
        if let Some(group_name) = executor_config.executor_group.as_deref()
            && last_maintenance.elapsed() >= executor_config.maintenance_interval
        {
            last_maintenance = Instant::now();
            run_group_maintenance(
                &stream_key,
                group_name,
                &app_instance_id,
                &executor_config,
                &handler,
            )
            .await;
        }

        if executor_config.scan_interval != ticker.period() {
            ticker = build_ticker(executor_config.scan_interval);
        }
    }
}

/// # 消费者组维护
///
/// 两步，顺序不能颠倒：
///
/// 1. **认领僵死消息**（`XAUTOCLAIM`）：把空闲超过 `stale` 的消息改派到当前实例名下。
///    这些消息通常是某个 worker 崩溃 / 被 kill / 网络分区时留下的 —— 它们已经被投递进
///    那个实例的 PEL，`">"` 永远不会再返回它们，不做认领就等于永久丢失。认领后走
///    [`handle_stream_message`]，与正常轮询共用完全相同的处理路径。
///
/// 2. **回收僵死消费者**（`XGROUP DELCONSUMER`）：`XREADGROUP` 会为每个出现过的消费者名
///    自动建档，而 Redis **不会**因为进程退出就自动清理它。每次滚动发布都会新增一个
///    消费者名（实例名含 IP + 端口），不回收的话消费者列表会无限增长。回收条件三条同时满足：
///    不是自己、空闲超过 `stale`、`pending == 0`。
///
/// 第 1 步排在第 2 步之前：某消费者若还挂着 pending 消息，本步会先把它认领走，使其
/// `pending` 归零，从而在下一轮维护（或本轮，因为 XAUTOCLAIM 是同步改派，返回时源
/// 消费者的 pending 已扣减）被回收。
///
/// 全程只记日志不抛错：维护失败不该影响正常的消息消费。
///
/// `noack` 为 `true` 时第 1 步必然扫不到东西（`NOACK` 的消息从不进 PEL），但依然保留：
/// 一是第 2 步回收僵死消费者仍然必要，二是配置热切换（`false → true`）时能把此前
/// 遗留在 PEL 里的消息清干净。
///
/// `last_only` 会原样透传给 [`dispatch_stream_messages`]：认领回来的往往正是
/// 「实例崩溃后遗留在 PEL 里的一批重复消息」，与正常轮询遇到积压是同一个场景，
/// 因此同样适用「只处理最后一条」。
async fn run_group_maintenance(
    stream_key: &str,
    group_name: &str,
    self_consumer: &str,
    config: &JobxExecutorConfig,
    handler: &Arc<dyn JobxTaskHandler>,
) {
    let min_idle_ms = config.consumer_stale_duration.as_millis() as usize;

    // ---- 1. 认领僵死消息并重跑 ----
    match auto_claim_stale_messages(
        stream_key,
        group_name,
        self_consumer,
        min_idle_ms,
        config.max_messages,
    )
    .await
    {
        Ok(reply) => {
            // deleted_ids 是「已从流里删除、但仍挂在 PEL 上」的孤儿 ID，
            // Redis 已在 XAUTOCLAIM 内把它们从 PEL 清掉，这里只记一笔。
            if !reply.deleted_ids.is_empty() {
                trace!(
                    "清理 PEL 孤儿消息 {} 条: {:?}",
                    reply.deleted_ids.len(),
                    reply.deleted_ids
                );
            }
            if !reply.claimed.is_empty() {
                warn!(
                    "认领到 {} 条僵死消息（空闲 > {}ms），重新处理: stream={stream_key}, group={group_name}",
                    reply.claimed.len(),
                    min_idle_ms,
                );
                dispatch_stream_messages(
                    stream_key,
                    reply.claimed,
                    self_consumer,
                    Some(group_name),
                    config,
                    handler,
                )
                .await;
            }
        }
        Err(e) => warn!("XAUTOCLAIM 失败: {e:?}"),
    }

    // ---- 2. 回收僵死消费者 ----
    let consumers = match list_group_consumers(stream_key, group_name).await {
        Ok(c) => c,
        Err(e) => {
            warn!("XINFO CONSUMERS 失败: {e:?}");
            return;
        }
    };
    for consumer in consumers {
        if consumer.name == self_consumer {
            continue;
        }
        // 还挂着未 ACK 的消息时不能删：DELCONSUMER 会把它的 PEL 一并丢弃，
        // 那些消息就此彻底失联。它们会在下一轮被 XAUTOCLAIM 认领走，之后自然可回收。
        if consumer.pending > 0 {
            continue;
        }
        if (consumer.idle as u128) < config.consumer_stale_duration.as_millis() {
            continue;
        }
        match delete_group_consumer(stream_key, group_name, &consumer.name).await {
            Ok(discarded) => info!(
                "回收僵死消费者: name={}, idle={}ms, 连带丢弃 pending={}",
                consumer.name, consumer.idle, discarded,
            ),
            Err(e) => warn!("回收消费者 {} 失败: {e:?}", consumer.name),
        }
    }
}

/// # 批量分发一批 Stream 消息
///
/// 正常轮询与消费者组维护（`XAUTOCLAIM` 认领回来的消息）**共用此函数**，
/// 保证两条路径的分发策略完全一致。
///
/// 默认逐条交给 [`handle_stream_message`] 并发处理。
///
/// `last_only` 为 `true` 时**只把批次里最后一条**交出去，其余就地丢弃。
/// 适用场景与代价见 [`JobxExecutorConfig::last_only`]。
///
/// ## 被丢弃的前缀怎么处置（取决于 `noack`）
///
/// - **`noack = true`（免应答）**：这些消息从未进入 PEL，既不需要 `XACK`，也没有别的
///   后续动作。此时**不逐条遍历**，只打一行汇总日志把它们整体丢弃 —— 逐条 `ack_message`
///   只是空转一次异步调用，纯粹浪费。这也正是「免应答」省下来的那部分开销。
/// - **`noack = false`（可靠消费）**：必须逐条 `XACK`。否则它们会留在 PEL 里，
///   被下一轮 [`run_group_maintenance`] 的 `XAUTOCLAIM` 认领回来重跑，
///   「只处理最后一条」就白开了。
async fn dispatch_stream_messages(
    stream_name: &str,
    stream_ids: Vec<StreamId>,
    instance_id: &str,
    executor_group: Option<&str>,
    config: &JobxExecutorConfig,
    handler: &Arc<dyn JobxTaskHandler>,
) {
    // 只有 `last_only` 才需要丢弃前缀；其余情况 `stale_count` 为 0，下面的分支全跳过。
    // 批次为空时 `saturating_sub` 得 0，不会越界。
    let stale_count = if config.last_only {
        stream_ids.len().saturating_sub(1)
    } else {
        0
    };

    if stale_count > 0 {
        if config.noack {
            // 免应答：不遍历、不 ACK，一行日志整体丢弃。
            info!(
                "只处理最后一条，丢弃 {} 条历史消息（noack 免应答）: stream={stream_name}",
                stale_count
            );
        } else {
            info!(
                "只处理最后一条，跳过并应答 {} 条历史消息: stream={stream_name}",
                stale_count
            );
            // 借出前缀逐条 ACK，循环结束即释放借用，随后才能 move `stream_ids`。
            for stream_id in &stream_ids[..stale_count] {
                ack_message(stream_name, executor_group, false, &stream_id.id).await;
            }
        }
    }

    // 前 `stale_count` 条已处置完毕（或直接丢弃），剩下的逐条并发处理。
    for stream_id in stream_ids.into_iter().skip(stale_count) {
        tokio::spawn(handle_stream_message(
            stream_name.to_string(),
            stream_id,
            instance_id.to_string(),
            executor_group.map(|g| g.to_string()),
            config.noack,
            Arc::clone(handler),
        ));
    }
}

/// # 处理单条 Stream 消息
///
/// 从消息 payload 解析出任务计划，调用 `take` 创建任务记录，执行 `handler`，上报结果。
/// **正常轮询与消费者组维护（`XAUTOCLAIM` 认领回来的消息）共用此函数**，
/// 保证两条路径的处理逻辑完全一致。
///
/// ## ACK 时机
///
/// **`take` 成功的那一刻就 ACK**，不等任务执行完。理由见函数内注释：消息只负责
/// 「唤醒一次调度动作」，该动作随 `take` 落库即完成；把它一直挂到任务结束会与
/// [`run_group_maintenance`] 的 XAUTOCLAIM 打架（长任务会被别的实例当僵死消息抢走重跑）。
///
/// ## 失败路径
///
/// 解析失败 / 取不到 API 客户端 / `take` 失败都会 ACK 掉消息，理由见 [`ack_message`]。
///
/// ## `noack`
///
/// 配置 `noack = true` 时消息从未进入 PEL，所有 `XACK` 都是多余的（虽然无害），
/// 因此整条路径上的 ACK 一律跳过。
async fn handle_stream_message(
    stream_name: String,
    stream_id: StreamId,
    instance_id: String,
    executor_group: Option<String>,
    noack: bool,
    handler: Arc<dyn JobxTaskHandler>,
) {
    info!("收到任务: stream={}, id={}", stream_name, stream_id.id);

    let job = match extract_job_from_message(&stream_id.map) {
        Ok(job) => job,
        Err(e) => {
            warn!("解析任务消息失败: id={}, error={}", stream_id.id, e);
            ack_message(
                &stream_name,
                executor_group.as_deref(),
                noack,
                &stream_id.id,
            )
            .await;
            return;
        }
    };

    let api_client = match get_jobx_api_client() {
        Ok(c) => c,
        Err(e) => {
            warn!("获取 API 客户端失败: {e:?}");
            ack_message(
                &stream_name,
                executor_group.as_deref(),
                noack,
                &stream_id.id,
            )
            .await;
            return;
        }
    };
    let user_id: U64 = job.updater_id.into();
    let add_dto = build_task_add_dto(&job, instance_id, user_id);
    let task = match api_client.task_client.take(&add_dto).await {
        Ok(ro) => match ro.extra {
            Some(t) => t,
            None => {
                warn!("领取任务失败: 返回数据为空");
                ack_message(
                    &stream_name,
                    executor_group.as_deref(),
                    noack,
                    &stream_id.id,
                )
                .await;
                return;
            }
        },
        Err(e) => {
            warn!("领取任务失败: {e:?}");
            ack_message(
                &stream_name,
                executor_group.as_deref(),
                noack,
                &stream_id.id,
            )
            .await;
            return;
        }
    };

    // ---- 消息的使命到此结束，立即 ACK ----
    //
    // `take` 成功即代表「这条消息要唤醒的调度动作已经完成」：服务端在同一事务里创建了
    // `jobx_task` 行并调用了 `recalc_next_assign_ms` 推进计划的下次分派时间。此后无论
    // 任务执行多久、成功还是失败，都与这条 Stream 消息无关了 —— 因此不能等到任务跑完
    // 才 ACK。
    //
    // 若把 ACK 拖到任务结束，消息会在 PEL 里滞留整个任务时长，与 `run_group_maintenance`
    // 的 XAUTOCLAIM 直接冲突：一个跑了几分钟的高频任务，会被其它实例当作「僵死消息」
    // 认领走并再执行一遍。ACK 前移把「消息在 PEL 里的存活窗口」压缩到毫秒级，
    // `consumer_stale_duration` 也就有了充足的余量。
    //
    // 这不影响可靠性：ACK 早并不丢任务 —— 崩溃恢复的粒度在 `jobx_task` 行上，
    // 而那一行此时已经落库。
    //
    // `noack = true` 时消息从未进入 PEL，没有「待确认」状态，跳过即可。
    if !noack
        && let Some(group_name) = executor_group.as_deref()
        && let Err(e) = ack_stream_message(&stream_name, group_name, &[&stream_id.id]).await
    {
        warn!("ACK 消息 {} 失败: {:?}", stream_id.id, e);
    }

    let task_id = task.id;

    // 如果设定了预定执行时间，延迟到该时间再执行
    if let Some(scheduled_ms) = task.scheduled_exec_start_ms {
        let scheduled_ms = U64::from(scheduled_ms).value();
        let now = now_ms();
        if scheduled_ms > now {
            info!("任务 {} 延迟到预定时间 {}ms 再执行", task_id, scheduled_ms);
            let deadline = Instant::now() + Duration::from_millis(scheduled_ms - now);
            tokio::time::sleep_until(deadline).await;
        }
    }

    // ---- 执行任务 ----
    let retry_config = get_jobx_executor_config(&task.executor_code).ok();
    let (retry_count, retry_interval) = retry_config
        .as_ref()
        .map(|c| (c.report_retry_count, c.report_retry_interval))
        .unwrap_or((3, Duration::from_secs(1)));

    // 标记开始执行
    let exec_start_ms = now_ms();
    let start_dto = JobxTaskModifyDto::builder()
        .id(task_id.into())
        .exec_start_ms(Some(exec_start_ms.into()))
        ._current_user_id(user_id)
        .build();
    if let Err(e) = api_client.task_client.modify(&start_dto).await {
        warn!("标记任务 {} 开始执行失败: {e:?}", task_id);
    }

    let is_high_freq = task.high_freq == Some(true);

    // 执行任务，收集状态和详情
    let (status, exec_detail) = if is_high_freq {
        let interval_ms = task
            .interval_duration
            .as_ref()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1000);
        let end_ms = task.valid_end_ms.map_or(u64::MAX, |v| u64::from(v));
        let is_fixed_rate = task.job_type == JobType::FixedRate;

        let strategy_desc = if is_fixed_rate {
            "固定频率"
        } else {
            "固定延迟"
        };
        info!(
            "高频任务 {} 开始循环执行 ({}, interval={}ms, end={}ms)",
            task_id, strategy_desc, interval_ms, end_ms
        );

        let task = Arc::new(task);
        let mut total_count: u64 = 0;
        let mut success_count: u64 = 0;
        let mut fail_count: u64 = 0;
        let mut min_elapsed: u64 = u64::MAX;
        let mut max_elapsed: u64 = 0;
        let mut total_elapsed: u64 = 0;
        let mut first_elapsed: Option<u64> = None;
        let mut last_elapsed: u64 = 0;
        let mut last_status = TaskStatus::Success;

        loop {
            let loop_start = now_ms();
            if loop_start >= end_ms {
                break;
            }

            let h = Arc::clone(&handler);
            let t = Arc::clone(&task);
            let join_result = tokio::spawn(async move { h.handle(&t).await }).await;
            let elapsed = now_ms() - loop_start;

            total_count += 1;
            total_elapsed += elapsed;
            if elapsed < min_elapsed {
                min_elapsed = elapsed;
            }
            if elapsed > max_elapsed {
                max_elapsed = elapsed;
            }
            if first_elapsed.is_none() {
                first_elapsed = Some(elapsed);
            }
            last_elapsed = elapsed;

            match join_result {
                Ok(Ok(_)) => {
                    success_count += 1;
                    last_status = TaskStatus::Success;
                }
                Ok(Err(e)) => {
                    fail_count += 1;
                    warn!("高频任务 {} 第{}次执行失败: {e:#}", task_id, total_count,);
                    last_status = TaskStatus::Failed;
                }
                Err(e) => {
                    fail_count += 1;
                    warn!("高频任务 {} 第{}次执行panic: {:?}", task_id, total_count, e,);
                    last_status = TaskStatus::Failed;
                }
            }

            let sleep_ms = if is_fixed_rate {
                if elapsed < interval_ms {
                    interval_ms - elapsed
                } else {
                    0
                }
            } else {
                interval_ms
            };

            if sleep_ms > 0 {
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
            }
        }

        let avg_elapsed = if total_count > 0 {
            total_elapsed / total_count
        } else {
            0
        };

        info!(
            "高频任务 {} 执行完成: 共{}次 成功{} 失败{} | \
             min={}ms max={}ms avg={}ms | 首次={}ms 末次={}ms",
            task_id,
            total_count,
            success_count,
            fail_count,
            min_elapsed,
            max_elapsed,
            avg_elapsed,
            first_elapsed.unwrap_or(0),
            last_elapsed,
        );

        let exec_detail = format!(
            "共{}次 成功{} 失败{} | min={}ms max={}ms avg={}ms | 首次={}ms 末次={}ms",
            total_count,
            success_count,
            fail_count,
            min_elapsed,
            max_elapsed,
            avg_elapsed,
            first_elapsed.unwrap_or(0),
            last_elapsed,
        );
        let status = last_status;
        (status, exec_detail)
    } else {
        let join_result = tokio::spawn(async move { handler.handle(&task).await }).await;
        match join_result {
            Ok(Ok(exec_detail)) => (TaskStatus::Success, exec_detail),
            Ok(Err(e)) => {
                warn!("任务 {} 执行失败: {e:#}", task_id);
                (TaskStatus::Failed, format!("{e:#}"))
            }
            Err(e) => {
                warn!("任务 {} 执行panic: {:?}", task_id, e);
                (TaskStatus::Failed, format!("panic: {e}"))
            }
        }
    };

    // 统一上报
    report_task_result(
        &api_client,
        U64::from(task_id),
        status,
        exec_detail,
        user_id,
        retry_count,
        retry_interval,
    )
    .await;
}

/// # 确认（ACK）一条不再需要处理的消息
///
/// 两个调用场景，语义都是「这条消息的使命已经结束，把它从 PEL 里摘掉」：
///
/// 1. **处理前就失败**：解析失败 / 取不到 API 客户端 / `take` 失败；
/// 2. **`last_only` 主动丢弃**的历史消息（见 [`dispatch_stream_messages`]）。
///
/// 必须 ACK 的原因：本模块只用 `">"` 读新消息，没有任何 PEL 重投机制，
/// 未 ACK 的消息既不会被重投、也不会被清理，PEL 只会单调膨胀。
/// 更直接的是 [`run_group_maintenance`] 里的 `XAUTOCLAIM` 会把它们认领回来重跑 ——
/// 对场景 2 来说那等于「只处理最后一条」完全失效。
///
/// ACK 掉不等于丢任务：这些消息对应的 `jobx_job.next_assign_ms` 没有被推进，
/// `JobxJobDao::list_publishable` 的 `NOT EXISTS(jobx_task...)` 条件依然成立，
/// 调度器下一轮扫描会重新发布（依赖发布侧已去掉 `XLEN==0` 门闸）。
///
/// 无消费者组（`group` 为 `None`）时无需 ACK，直接返回；
/// `noack` 为 `true` 时消息从未进入 PEL，同样无需 ACK。
async fn ack_message(stream_name: &str, group: Option<&str>, noack: bool, message_id: &str) {
    if noack {
        return;
    }
    let Some(group_name) = group else {
        return;
    };
    if let Err(e) = ack_stream_message(stream_name, group_name, &[message_id]).await {
        warn!("ACK 消息 {message_id} 失败: {e:?}");
    }
}

/// # 带重试的上报任务执行结果
///
/// 根据配置的重试次数和重试间隔，将任务执行结果上报到服务端。
/// 重试耗尽后仍失败则仅记录告警日志，不再抛出错误。
async fn report_task_result(
    api_client: &Arc<JobxApiClient>,
    task_id: U64,
    status: TaskStatus,
    exec_detail: String,
    user_id: U64,
    retry_count: u32,
    retry_interval: Duration,
) {
    let dto = JobxTaskModifyDto::builder()
        .id(task_id.into())
        .status(status)
        .exec_detail(Some(exec_detail))
        .exec_end_ms(Some(now_ms().into()))
        ._current_user_id(user_id)
        .build();

    for attempt in 0..=retry_count {
        match api_client.task_client.modify(&dto).await {
            Ok(_) => return,
            Err(e) if attempt < retry_count => {
                warn!(
                    "上报任务 {} 执行结果失败（第{}次重试）: {e:?}",
                    task_id,
                    attempt + 1
                );
                tokio::time::sleep(retry_interval).await;
            }
            Err(e) => {
                warn!(
                    "上报任务 {} 执行结果失败（已重试{}次）: {e:?}",
                    task_id, retry_count,
                );
                return;
            }
        }
    }
}

/// # 从 Redis Stream 消息中提取 JobxJobVo
///
/// 从消息的 `payload` 字段中取出 JSON 字符串，反序列化为任务计划视图对象。
fn extract_job_from_message(fields: &HashMap<String, RedisValue>) -> Result<JobxJobVo, String> {
    let payload = fields
        .get("payload")
        .and_then(|v| match v {
            RedisValue::BulkString(bytes) => std::str::from_utf8(bytes).ok(),
            _ => None,
        })
        .ok_or_else(|| "payload 字段缺失或格式错误".to_string())?;

    serde_json::from_str(payload).map_err(|e| format!("JSON 反序列化失败: {e}"))
}

fn build_stream_key(executor_key: &str, executor_code: &str) -> String {
    format!("{}:{}", executor_key, executor_code)
}
