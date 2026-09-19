use crate::config::{
    get_jobx_worker_config, set_jobx_worker_config, JobxWorkerConfig, JOBX_WORKER_CONFIG_KEY,
};
use async_trait::async_trait;
use config::Value;
use jobx_api::dic::{TaskStatus, TaskType};
use jobx_api::vo::JobxJobVo;
use jobx_api_client::api_client::{get_jobx_api_client, setup_jobx_api_client};
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
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::time_utils::{build_ticker, now_ms};

/// # 消息处理 trait
///
/// Worker 从 Redis Stream 收到任务消息后，自动将 payload 反序列化为 `JobxJobVo`
/// 并调用此 trait 进行处理。
/// 下游客户端需实现此 trait 来完成实际的任务执行逻辑。
///
/// ## 使用示例
///
/// ```no_run
/// use async_trait::async_trait;
/// use jobx_api::vo::JobxJobVo;
/// use jobx_wkr::JobxMessageHandler;
///
/// struct MyHandler;
///
/// #[async_trait]
/// impl JobxMessageHandler for MyHandler {
///     async fn handle(&self, job: &JobxJobVo) {
///         // job 包含任务计划的所有信息：executor_code、params、job_type 等
///         // 下游客户端根据 job 信息执行具体业务逻辑
///     }
/// }
/// ```
#[async_trait]
pub trait JobxMessageHandler: Send + Sync + 'static {
    /// 处理从 Redis Stream 收到的任务
    /// - job: 反序列化后的任务计划视图对象，包含 executor_code、params、job_type 等完整字段
    async fn handle(&self, job: &JobxJobVo);
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
                    let user_id = job.updator_id;

                    // 创建任务记录
                    let task_type = if job.next_assign_ms.is_some() {
                        TaskType::Scheduled
                    } else {
                        TaskType::Immediate
                    };
                    let assign_ms = now_ms();
                    let add_dto = JobxTaskAddDto::builder()
                        .task_type(task_type)
                        .job_id(Some(U64(job.id)))
                        .assign_ms(U64(assign_ms))
                        .scheduled_exec_start_ms(job.next_assign_ms.map(U64))
                        .build();
                    let task = match api_client.task_client.add(&add_dto, user_id).await {
                        Ok(ro) => match ro.extra {
                            Some(t) => t,
                            None => {
                                warn!("创建任务记录失败: 返回数据为空");
                                return;
                            }
                        },
                        Err(e) => {
                            warn!("创建任务记录失败: {e:?}");
                            return;
                        }
                    };
                    let task_id = task.id;

                    // 如果设定了预定执行时间，延迟到该时间再执行
                    if let Some(scheduled_ms) = job.next_assign_ms {
                        let now = now_ms();
                        if scheduled_ms > now {
                            let delay_ms = scheduled_ms - now;
                            info!("任务 {} 延迟 {}ms 到预定时间再执行", task_id, delay_ms);
                            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        }
                    }

                    // 标记任务开始执行
                    let exec_start_ms = now_ms();
                    let start_dto = JobxTaskModifyDto::builder()
                        .id(U64(task_id))
                        .exec_start_ms(Some(U64(exec_start_ms)))
                        .build();
                    if let Err(e) = api_client.task_client.modify(&start_dto, user_id).await {
                        warn!("标记任务 {} 开始执行失败: {e:?}", task_id);
                    }

                    // 执行业务逻辑
                    let handle_result =
                        tokio::spawn(async move { handler.handle(&job).await }).await;

                    // 上报执行结果
                    match handle_result {
                        Ok(()) => {
                            info!("任务 {} 执行成功", task_id);
                            let success_dto = JobxTaskModifyDto::builder()
                                .id(U64(task_id))
                                .status(TaskStatus::Success)
                                .exec_end_ms(Some(U64(now_ms())))
                                .build();
                            if let Err(e) =
                                api_client.task_client.modify(&success_dto, user_id).await
                            {
                                warn!("上报任务 {} 执行结果失败: {e:?}", task_id);
                            }
                        }
                        Err(e) => {
                            let err_msg = if e.is_panic() {
                                "任务执行发生 panic".to_string()
                            } else {
                                "任务被取消".to_string()
                            };
                            warn!("任务 {} 执行失败: {}", task_id, err_msg);
                            let fail_dto = JobxTaskModifyDto::builder()
                                .id(U64(task_id))
                                .status(TaskStatus::Failed)
                                .exec_detail(Some(err_msg))
                                .exec_end_ms(Some(U64(now_ms())))
                                .build();
                            if let Err(e) = api_client.task_client.modify(&fail_dto, user_id).await
                            {
                                warn!("上报任务 {} 执行结果失败: {e:?}", task_id);
                            }
                        }
                    }
                });
            }
        }

        if worker_config.scan_interval != ticker.period() {
            ticker = build_ticker(worker_config.scan_interval);
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
