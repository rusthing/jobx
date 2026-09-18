use crate::config::{
    get_jobx_worker_config, set_jobx_worker_config, JobxWorkerConfig, JOBX_WORKER_CONFIG_KEY,
};
use config::Value;
use robotech::cfg::CfgError;
use robotech::env::{AppEnv, EnvError, APP_ENV};
use robotech::redis::{ensure_consumer_group, read_from_stream};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::time_utils::build_ticker;

pub async fn setup_jobx_worker(
    worker_config: JobxWorkerConfig,
    changed: &Option<HashMap<String, Value>>,
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
            ));
        }
    }
    Ok(())
}

/// # 启动工作者订阅循环
///
/// 订阅 Redis Stream 中发布的任务，阻塞等待新消息到达。
/// 运行期间会动态读取最新配置，若扫描间隔变更则自动调整 ticker。
/// 该函数不会返回，需通过 `tokio::spawn` 在独立任务中运行。
async fn run_worker_loop(app_instance_id: String, initial_scan_interval: Duration) {
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
                info!(
                    "收到任务: stream={}, id={}, fields={:?}",
                    stream_key.key, stream_id.id, stream_id.map
                );
            }
        }

        if worker_config.scan_interval != ticker.period() {
            ticker = build_ticker(worker_config.scan_interval);
        }
    }
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
