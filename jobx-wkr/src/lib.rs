//! # JobX Executor Client Library
//!
//! 为任务执行器（Executor）提供的简化工具函数，封装了与 JobX 服务端的 HTTP 通信，
//! 提供拉取待执行任务、上报执行结果等常用操作。
//!
//! ## 使用示例
//!
//! ```no_run
//! use jobx_wkr::{create_client, fetch_pending_tasks, start_task, report_success, JobxExecutorConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = JobxExecutorConfig {
//!         base_url: "http://127.0.0.1:8080".to_string(),
//!         user_id: 1,
//!     };
//!     let client = create_client(&config).await?;
//!
//!     // 拉取待执行任务
//!     let tasks = fetch_pending_tasks(&client, config.user_id, "my-job").await?;
//!
//!     for task in tasks {
//!         let task_id: u64 = task.id.into();
//!         start_task(&client, config.user_id, task_id).await?;
//!
//!         // 执行业务逻辑...
//!
//!         report_success(&client, config.user_id, task_id, Some("执行成功")).await?;
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod utils;

pub use config::JobxExecutorConfig;
pub use utils::{setup_jobx_executor, JobxMessageHandler};