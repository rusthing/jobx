//! # JobX Worker Client Library
//!
//! 为任务执行器（Worker）提供的简化客户端库，封装了与 JobX 服务端的 HTTP 通信，
//! 提供拉取待执行任务、上报执行结果等常用操作。
//!
//! ## 使用示例
//!
//! ```no_run
//! use jobx_wkr::{JobxWkr, WkrConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let wkr = JobxWkr::new(WkrConfig {
//!         base_url: "http://127.0.0.1:8080".to_string(),
//!         user_id: 1,
//!     }).await?;
//!
//!     // 拉取待执行任务
//!     let tasks = wkr.fetch_pending_tasks("my-job").await?;
//!
//!     for task in tasks {
//!         let task_id: u64 = task.id.into();
//!         wkr.start_task(task_id).await?;
//!
//!         // 执行业务逻辑...
//!
//!         wkr.report_success(task_id, Some("执行成功")).await?;
//!     }
//!
//!     Ok(())
//! }
//! ```

use jobx_api::vo::{JobxJobVo, JobxTaskVo};
use reqwest::header::{HeaderMap, HeaderValue};
use robotech::api::Ro;
use robotech::api_client::{ApiClientConfig, ApiClientError};
use robotech::micro_svc::FeignApiClient;
use wheel_rs::time_utils::now_ms;

/// Worker 配置
#[derive(Debug, Clone)]
pub struct WkrConfig {
    /// JobX 服务端 base URL（例如: `http://127.0.0.1:8080`）
    pub base_url: String,
    /// Worker 用户 ID，用于请求头鉴权（`X-User-Id`）
    pub user_id: u64,
}

/// 任务执行器客户端
///
/// 封装了与 JobX 服务端的通信，提供拉取任务、标记执行、上报结果等简化接口。
///
/// ## 任务生命周期
///
/// 1. `fetch_pending_tasks` / `fetch_one_pending_task` — 拉取待执行任务
/// 2. `start_task` — 标记任务开始执行
/// 3. `report_success` / `report_failure` — 上报执行结果
pub struct JobxWkr {
    client: FeignApiClient,
    user_id: u64,
}

impl JobxWkr {
    /// 创建新的 Worker 实例
    ///
    /// 内部使用 `ApiClientConfig::Simple` 直连模式连接 JobX 服务端。
    pub async fn new(config: WkrConfig) -> Result<Self, ApiClientError> {
        let api_config = ApiClientConfig::Simple {
            base_url: config.base_url,
            auth: None,
        };
        Ok(Self {
            client: FeignApiClient::new(api_config).await,
            user_id: config.user_id,
        })
    }

    /// 构建包含用户 ID 的请求头
    fn build_headers(&self) -> Result<HeaderMap, ApiClientError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-User-Id",
            HeaderValue::from_str(&self.user_id.to_string())
                .map_err(|e| ApiClientError::NotInit(format!("invalid user_id: {e}")))?,
        );
        Ok(headers)
    }

    /// 根据 job executor_code 获取 job 信息
    ///
    /// ## 参数
    /// * `code` - 任务的计划编码（对应 `JobxJobDto.executor_code` 字段）
    pub async fn get_job_by_code(&self, code: &str) -> Result<JobxJobVo, ApiClientError> {
        let params = serde_json::json!({ "executor_code": code });
        let headers = self.build_headers()?;
        let ro: Ro<JobxJobVo> = self
            .client
            .request(
                reqwest::Method::GET,
                "/jobx/jobx-job",
                Some(&params),
                None::<&serde_json::Value>,
                Some(&headers),
            )
            .await?;

        ro.extra
            .ok_or_else(|| ApiClientError::NotInit(format!("job not found by code: {code}")))
    }

    /// 拉取指定 job 下所有待执行（状态为 Running）的任务
    ///
    /// ## 参数
    /// * `executor_code` - 任务的计划编码
    pub async fn fetch_pending_tasks(
        &self,
        executor_code: &str,
    ) -> Result<Vec<JobxTaskVo>, ApiClientError> {
        let job = self.get_job_by_code(executor_code).await?;
        let job_id = job.id;

        let params = serde_json::json!({
            "jobId": job_id,
            "status": 0,
        });
        let headers = self.build_headers()?;
        let ro: Ro<Vec<JobxTaskVo>> = self
            .client
            .request(
                reqwest::Method::GET,
                "/jobx/jobx-task/list",
                Some(&params),
                None::<&serde_json::Value>,
                Some(&headers),
            )
            .await?;

        Ok(ro.extra.unwrap_or_default())
    }

    /// 拉取一个待执行的任务，没有则返回 `None`
    ///
    /// ## 参数
    /// * `executor_code` - 任务的计划编码
    pub async fn fetch_one_pending_task(
        &self,
        executor_code: &str,
    ) -> Result<Option<JobxTaskVo>, ApiClientError> {
        let mut tasks = self.fetch_pending_tasks(executor_code).await?;
        Ok(if tasks.is_empty() {
            None
        } else {
            Some(tasks.remove(0))
        })
    }

    /// 标记任务开始执行
    ///
    /// 更新任务的 `exec_start_ms` 字段为当前时间戳。
    ///
    /// ## 参数
    /// * `task_id` - 任务 ID
    pub async fn start_task(&self, task_id: u64) -> Result<(), ApiClientError> {
        let body = serde_json::json!({
            "id": task_id,
            "execStartTs": now_ms(),
        });
        let headers = self.build_headers()?;
        self.client
            .request::<serde_json::Value, Ro<JobxTaskVo>>(
                reqwest::Method::PUT,
                "/jobx/jobx-task",
                None::<&serde_json::Value>,
                Some(&body),
                Some(&headers),
            )
            .await?;
        Ok(())
    }

    /// 上报任务执行成功
    ///
    /// 更新任务的 `status` 为 Success（1），同时记录 `exec_detail` 和 `exec_end_ms`。
    ///
    /// ## 参数
    /// * `task_id` - 任务 ID
    /// * `exec_detail` - 执行详情（可选）
    pub async fn report_success(
        &self,
        task_id: u64,
        exec_detail: Option<&str>,
    ) -> Result<(), ApiClientError> {
        let body = serde_json::json!({
            "id": task_id,
            "status": 1,
            "execDetail": exec_detail,
            "execEndMs": now_ms(),
        });
        let headers = self.build_headers()?;
        self.client
            .request::<serde_json::Value, Ro<JobxTaskVo>>(
                reqwest::Method::PUT,
                "/jobx/jobx-task",
                None::<&serde_json::Value>,
                Some(&body),
                Some(&headers),
            )
            .await?;
        Ok(())
    }

    /// 上报任务执行失败
    ///
    /// 更新任务的 `status` 为 Failed（2），同时记录 `exec_detail` 和 `exec_end_ms`。
    ///
    /// ## 参数
    /// * `task_id` - 任务 ID
    /// * `exec_detail` - 失败详情（错误信息）
    pub async fn report_failure(
        &self,
        task_id: u64,
        exec_detail: &str,
    ) -> Result<(), ApiClientError> {
        let body = serde_json::json!({
            "id": task_id,
            "status": 2,
            "execDetail": exec_detail,
            "execEndMs": now_ms(),
        });
        let headers = self.build_headers()?;
        self.client
            .request::<serde_json::Value, Ro<JobxTaskVo>>(
                reqwest::Method::PUT,
                "/jobx/jobx-task",
                None::<&serde_json::Value>,
                Some(&body),
                Some(&headers),
            )
            .await?;
        Ok(())
    }
}
