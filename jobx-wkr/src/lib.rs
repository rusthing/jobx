//! # JobX Executor Client Library
//!
//! 为任务执行器（Executor）提供的封装：从 Redis Stream 接收调度消息、创建任务记录、
//! 调用业务 handler 执行，并把结果回报给 JobX 服务端。
//!
//! ## 使用示例
//!
//! ```no_run
//! use async_trait::async_trait;
//! use jobx_api::vo::JobxTaskVo;
//! use jobx_wkr::{JobxExecutorConfig, JobxTaskHandler, init_jobx_executor};
//! use std::sync::Arc;
//!
//! struct MyHandler;
//!
//! #[async_trait]
//! impl JobxTaskHandler for MyHandler {
//!     async fn handle(&self, _task: &JobxTaskVo) -> anyhow::Result<String> {
//!         // 执行业务逻辑...
//!         Ok("执行成功".to_string())
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = JobxExecutorConfig::builder()
//!         .executor_code("my-executor".to_string())
//!         .build();
//!     init_jobx_executor(config, Arc::new(MyHandler)).await?;
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod utils;

pub use config::*;
pub use utils::*;
