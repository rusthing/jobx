use crate::config::{
    get_jobx_worker_config, set_jobx_worker_config, JobxWorkerConfig, JOBX_WORKER_CONFIG_KEY,
};
use async_trait::async_trait;
use config::Value;
use jobx_api::vo::JobxJobVo;
use redis::Value as RedisValue;
use robotech::cfg::CfgError;
use robotech::env::{AppEnv, EnvError, APP_ENV};
use robotech::redis::{ensure_consumer_group, read_from_stream};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::time_utils::build_ticker;

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
                    handler.handle(&job).await;
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

// /// 构建包含用户 ID 的请求头
// fn build_headers(user_id: u64) -> Result<HeaderMap, ApiClientError> {
//     let mut headers = HeaderMap::new();
//     headers.insert(
//         "X-User-Id",
//         HeaderValue::from_str(&user_id.to_string())
//             .map_err(|e| ApiClientError::NotInit(format!("invalid user_id: {e}")))?,
//     );
//     Ok(headers)
// }
//
// /// 创建 FeignApiClient 实例
// ///
// /// 内部使用 `ApiClientConfig::Simple` 直连模式连接 JobX 服务端。
// pub async fn create_client(config: &JobxWorkerConfig) -> Result<FeignApiClient, ApiClientError> {
//     let api_config = ApiClientConfig::Simple {
//         base_url: config.base_url.clone(),
//         auth: None,
//     };
//     Ok(FeignApiClient::new(api_config).await)
// }
//
// /// # 根据 job executor_code 获取 job 信息
// /// - client: HTTP 客户端
// /// - user_id: 用户 ID
// /// - code: 任务的计划编码（对应 `JobxJobDto.executor_code` 字段）
// pub async fn get_job_by_code(
//     client: &FeignApiClient,
//     user_id: u64,
//     code: &str,
// ) -> Result<JobxJobVo, ApiClientError> {
//     let params = serde_json::json!({ "executor_code": code });
//     let headers = build_headers(user_id)?;
//     let ro: Ro<JobxJobVo> = client
//         .request(
//             reqwest::Method::GET,
//             "/jobx/jobx-job",
//             Some(&params),
//             None::<&serde_json::Value>,
//             Some(&headers),
//         )
//         .await?;
//
//     ro.extra
//         .ok_or_else(|| ApiClientError::NotInit(format!("job not found by code: {code}")))
// }
//
// /// # 拉取指定 job 下所有待执行（状态为 Running）的任务
// /// - client: HTTP 客户端
// /// - user_id: 用户 ID
// /// - executor_code: 任务的计划编码
// pub async fn fetch_pending_tasks(
//     client: &FeignApiClient,
//     user_id: u64,
//     executor_code: &str,
// ) -> Result<Vec<JobxTaskVo>, ApiClientError> {
//     let job = get_job_by_code(client, user_id, executor_code).await?;
//     let job_id = job.id;
//
//     let params = serde_json::json!({
//         "jobId": job_id,
//         "status": 0,
//     });
//     let headers = build_headers(user_id)?;
//     let ro: Ro<Vec<JobxTaskVo>> = client
//         .request(
//             reqwest::Method::GET,
//             "/jobx/jobx-task/list",
//             Some(&params),
//             None::<&serde_json::Value>,
//             Some(&headers),
//         )
//         .await?;
//
//     Ok(ro.extra.unwrap_or_default())
// }
//
// /// # 拉取一个待执行的任务，没有则返回 `None`
// /// - client: HTTP 客户端
// /// - user_id: 用户 ID
// /// - executor_code: 任务的计划编码
// pub async fn fetch_one_pending_task(
//     client: &FeignApiClient,
//     user_id: u64,
//     executor_code: &str,
// ) -> Result<Option<JobxTaskVo>, ApiClientError> {
//     let mut tasks = fetch_pending_tasks(client, user_id, executor_code).await?;
//     Ok(if tasks.is_empty() {
//         None
//     } else {
//         Some(tasks.remove(0))
//     })
// }
//
// /// # 标记任务开始执行
// ///
// /// 更新任务的 `exec_start_ms` 字段为当前时间戳。
// /// - client: HTTP 客户端
// /// - user_id: 用户 ID
// /// - task_id: 任务 ID
// pub async fn start_task(
//     client: &FeignApiClient,
//     user_id: u64,
//     task_id: u64,
// ) -> Result<(), ApiClientError> {
//     let body = serde_json::json!({
//         "id": task_id,
//         "execStartTs": now_ms(),
//     });
//     let headers = build_headers(user_id)?;
//     client
//         .request::<serde_json::Value, Ro<JobxTaskVo>>(
//             reqwest::Method::PUT,
//             "/jobx/jobx-task",
//             None::<&serde_json::Value>,
//             Some(&body),
//             Some(&headers),
//         )
//         .await?;
//     Ok(())
// }
//
// /// # 上报任务执行成功
// ///
// /// 更新任务的 `status` 为 Success（1），同时记录 `exec_detail` 和 `exec_end_ms`。
// /// - client: HTTP 客户端
// /// - user_id: 用户 ID
// /// - task_id: 任务 ID
// /// - exec_detail: 执行详情（可选）
// pub async fn report_success(
//     client: &FeignApiClient,
//     user_id: u64,
//     task_id: u64,
//     exec_detail: Option<&str>,
// ) -> Result<(), ApiClientError> {
//     let body = serde_json::json!({
//         "id": task_id,
//         "status": 1,
//         "execDetail": exec_detail,
//         "execEndMs": now_ms(),
//     });
//     let headers = build_headers(user_id)?;
//     client
//         .request::<serde_json::Value, Ro<JobxTaskVo>>(
//             reqwest::Method::PUT,
//             "/jobx/jobx-task",
//             None::<&serde_json::Value>,
//             Some(&body),
//             Some(&headers),
//         )
//         .await?;
//     Ok(())
// }
//
// /// # 上报任务执行失败
// ///
// /// 更新任务的 `status` 为 Failed（2），同时记录 `exec_detail` 和 `exec_end_ms`。
// /// - client: HTTP 客户端
// /// - user_id: 用户 ID
// /// - task_id: 任务 ID
// /// - exec_detail: 失败详情（错误信息）
// pub async fn report_failure(
//     client: &FeignApiClient,
//     user_id: u64,
//     task_id: u64,
//     exec_detail: &str,
// ) -> Result<(), ApiClientError> {
//     let body = serde_json::json!({
//         "id": task_id,
//         "status": 2,
//         "execDetail": exec_detail,
//         "execEndMs": now_ms(),
//     });
//     let headers = build_headers(user_id)?;
//     client
//         .request::<serde_json::Value, Ro<JobxTaskVo>>(
//             reqwest::Method::PUT,
//             "/jobx/jobx-task",
//             None::<&serde_json::Value>,
//             Some(&body),
//             Some(&headers),
//         )
//         .await?;
//     Ok(())
// }