use crate::config::{
    get_jobx_worker_config, set_jobx_worker_config, JobxWorkerConfig, JOBX_WORKER_CONFIG_KEY,
};
use async_trait::async_trait;
use config::Value;
use jobx_api::dic::{JobType, TaskStatus, TaskType};
use jobx_api::vo::{JobxJobVo, JobxTaskVo};
use jobx_api_client::api_client::{get_jobx_api_client, setup_jobx_api_client, JobxApiClient};
use jobx_api_client::dto::{JobxTaskAddDto, JobxTaskModifyDto};
use redis::Value as RedisValue;
use robotech::api::U64;
use robotech::api_client::ApiClientConfig;
use robotech::cfg::CfgError;
use robotech::env::{AppEnv, EnvError, APP_ENV};
use robotech::redis::{ensure_consumer_group, read_from_stream};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::time_utils::{build_ticker, now_ms};

/// 从 JobxJobVo 构建 JobxTaskAddDto
///
/// 将任务计划中的通用属性（executor_code、exec_params、job_type、cron 等）
/// 映射到任务记录 DTO，避免在 worker 循环中重复手写属性赋值。
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
                .map(|(next_ms, lead_ms)| U64::from(u64::from(next_ms) + lead_ms)),
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
        .assign_lead_ms(job.assign_lead_ms.map(U64::from))
        ._current_user_id(user_id)
        .build()
}

/// # 消息处理 trait
///
/// Worker 从 Redis Stream 收到任务消息后，自动创建任务记录并反序列化为 `JobxTaskVo`，
/// 并调用此 trait 进行处理。
/// 下游客户端需实现此 trait 来完成实际的任务执行逻辑。
///
/// ## 使用示例
///
/// ```no_run
/// use async_trait::async_trait;
/// use jobx_api::vo::JobxTaskVo;
/// use jobx_wkr::JobxMessageHandler;
///
/// struct MyHandler;
///
/// #[async_trait]
/// impl JobxMessageHandler for MyHandler {
///     async fn handle(&self, task: &JobxTaskVo) {
///         // task 包含任务的所有信息：executor_code、exec_params、job_type、task_id 等
///         // 下游客户端根据 task 信息执行具体业务逻辑
///     }
/// }
/// ```
#[async_trait]
pub trait JobxMessageHandler: Send + Sync + 'static {
    /// 处理任务
    /// - task: 任务视图对象，包含 executor_code、exec_params、job_type、task_id 等完整字段
    async fn handle(&self, task: &JobxTaskVo);
}

pub async fn setup_jobx_worker(
    worker_config: JobxWorkerConfig,
    apis_config: HashMap<String, ApiClientConfig>,
    changed: &Option<HashMap<String, Value>>,
    handler: Arc<dyn JobxMessageHandler>,
) -> Result<(), CfgError> {
    info!("setup worker config...: {worker_config:?}");
    if changed
        .as_ref()
        .map(|changed| has_config_changed(JOBX_WORKER_CONFIG_KEY, changed))
        .unwrap_or(true)
    {
        set_jobx_worker_config(worker_config.clone());

        // 初始化 API 客户端
        setup_jobx_api_client(apis_config, changed).await?;

        // 如果是首次配置，启动扫描线程
        if changed.is_none() {
            let AppEnv {
                instance_id: app_instance_id,
                ..
            } = APP_ENV.get().ok_or(EnvError::GetAppEnv())?;
            // 确保消费者组存在
            if let Some(executor_group) = &worker_config.executor_group {
                let stream_key =
                    build_stream_key(&worker_config.executor_key, &worker_config.executor_code);
                ensure_consumer_group(&stream_key, executor_group)
                    .await
                    .map_err(|e| CfgError::Init(e.to_string()))?;
            }
            // 启动扫描线程
            tokio::spawn(run_worker_loop(
                app_instance_id.to_string(),
                worker_config.scan_interval,
                handler,
            ));
        }
    }
    Ok(())
}

/// # 启动工作者订阅循环
///
/// 订阅 Redis Stream 中发布的任务，阻塞等待新消息到达。
/// 收到消息后调用 `handler` 处理，每条消息独立处理，便于下游客户端实现具体业务逻辑。
/// 运行期间会动态读取最新配置，若扫描间隔变更则自动调整 ticker。
/// 该函数不会返回，需通过 `tokio::spawn` 在独立任务中运行。
async fn run_worker_loop(
    app_instance_id: String,
    initial_scan_interval: Duration,
    handler: Arc<dyn JobxMessageHandler>,
) {
    let mut ticker = build_ticker(initial_scan_interval);
    loop {
        ticker.tick().await;
        let worker_config = match get_jobx_worker_config() {
            Ok(c) => c,
            Err(e) => {
                warn!("获取工作者配置失败: {e:?}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        let stream_key =
            build_stream_key(&worker_config.executor_key, &worker_config.executor_code);
        let group = worker_config
            .executor_group
            .as_ref()
            .map(|g| (g.as_str(), app_instance_id.as_str()));
        let scan_block_duration = worker_config.scan_block_duration;
        let reply = match read_from_stream(
            &stream_key,
            ">",
            worker_config.max_messages,
            Some(&scan_block_duration),
            group,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("读取 Redis Stream 失败: {e:?}");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        for stream_key in reply.keys {
            for stream_id in stream_key.ids {
                let handler = Arc::clone(&handler);
                let stream_name = stream_key.key.clone();
                let instance_id = app_instance_id.clone();
                tokio::spawn(async move {
                    info!("收到任务: stream={}, id={}", stream_name, stream_id.id);

                    let job = match extract_job_from_message(&stream_id.map) {
                        Ok(job) => job,
                        Err(e) => {
                            warn!("解析任务消息失败: id={}, error={}", stream_id.id, e);
                            return;
                        }
                    };

                    let api_client = match get_jobx_api_client() {
                        Ok(c) => c,
                        Err(e) => {
                            warn!("获取 API 客户端失败: {e:?}");
                            return;
                        }
                    };
                    let user_id: U64 = job.updator_id.into();
                    let add_dto = build_task_add_dto(&job, instance_id, user_id);
                    let task = match api_client.task_client.take(&add_dto).await {
                        Ok(ro) => match ro.extra {
                            Some(t) => t,
                            None => {
                                warn!("领取任务失败: 返回数据为空");
                                return;
                            }
                        },
                        Err(e) => {
                            warn!("领取任务失败: {e:?}");
                            return;
                        }
                    };

                    let task_id = task.id;

                    // 如果设定了预定执行时间，延迟到该时间再执行
                    if let Some(scheduled_ms) = task.scheduled_exec_start_ms {
                        let scheduled_ms = U64::from(scheduled_ms).value();
                        let now = now_ms();
                        if scheduled_ms > now {
                            info!("任务 {} 延迟到预定时间 {}ms 再执行", task_id, scheduled_ms);
                            let deadline =
                                Instant::now() + Duration::from_millis(scheduled_ms - now);
                            tokio::time::sleep_until(deadline).await;
                        }
                    }

                    // ---- 执行任务 ----
                    let retry_config = get_jobx_worker_config().ok();
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
                        let mut exec_details = Vec::new();
                        let mut last_status = TaskStatus::Success;

                        loop {
                            let loop_start = now_ms();
                            if loop_start >= end_ms {
                                break;
                            }

                            let h = Arc::clone(&handler);
                            let t = Arc::clone(&task);
                            let join_result =
                                tokio::spawn(async move { h.handle(&t).await }).await;
                            let elapsed = now_ms() - loop_start;

                            match join_result {
                                Ok(_) => {
                                    exec_details.push(elapsed.to_string());
                                    last_status = TaskStatus::Success;
                                }
                                Err(e) => {
                                    warn!(
                                        "高频任务 {} 第{}次执行panic: {:?}",
                                        task_id,
                                        exec_details.len() + 1,
                                        e,
                                    );
                                    exec_details.push(format!("{}(panic)", elapsed));
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

                        info!(
                            "高频任务 {} 执行完成: 共{}次",
                            task_id,
                            exec_details.len()
                        );
                        let detail =
                            Some(format!("{}次: {}ms", exec_details.len(), exec_details.join(",")));
                        let status = last_status;
                        (status, detail)
                    } else {
                        let join_result =
                            tokio::spawn(async move { handler.handle(&task).await }).await;
                        match join_result {
                            Ok(_) => (TaskStatus::Success, None),
                            Err(e) => {
                                warn!("任务 {} 执行panic: {:?}", task_id, e);
                                (TaskStatus::Failed, Some(format!("panic: {e}")))
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
                });
            }
        }

        if worker_config.scan_interval != ticker.period() {
            ticker = build_ticker(worker_config.scan_interval);
        }
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
    exec_detail: Option<String>,
    user_id: U64,
    retry_count: u32,
    retry_interval: Duration,
) {
    let dto = JobxTaskModifyDto::builder()
        .id(task_id.into())
        .status(status)
        .exec_detail(exec_detail)
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