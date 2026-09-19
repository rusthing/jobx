use arc_swap::ArcSwapOption;
use robotech::cfg::CfgError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use wheel_rs::serde::duration_serde;

pub const JOBX_WORKER_CONFIG_KEY: &str = "jobx.worker";
static JOBX_WORKER_CONFIG: ArcSwapOption<JobxWorkerConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_worker_config() -> Result<Arc<JobxWorkerConfig>, CfgError> {
    JOBX_WORKER_CONFIG.load_full().ok_or(CfgError::NotInit(
        "Worker config not initialized".to_string(),
    ))
}

pub fn set_jobx_worker_config(config: JobxWorkerConfig) {
    JOBX_WORKER_CONFIG.store(Some(Arc::new(config.clone())));
}

/// Worker 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct JobxWorkerConfig {
    /// Redis Stream 的 key的前缀，后面跟着任务执行器的编码
    #[serde(default = "executor_key_default")]
    pub executor_key: String,
    /// 任务执行器的编码
    pub executor_code: String,
    /// 任务执行器订阅消息分组
    #[serde(default = "executor_group_default")]
    pub executor_group: Option<String>,
    /// 扫描间隔
    #[serde(with = "duration_serde", default = "scan_interval_default")]
    pub scan_interval: Duration,
    /// 扫描阻塞时间
    #[serde(with = "duration_serde", default = "scan_block_default")]
    pub scan_block_duration: Duration,
    /// 单次接收消息的最大数量
    #[serde(default = "max_messages_default")]
    pub max_messages: usize,
    /// 上报执行结果失败时最大重试次数
    #[serde(default = "report_retry_count_default")]
    pub report_retry_count: u32,
    /// 上报执行结果重试间隔
    #[serde(with = "duration_serde", default = "report_retry_interval_default")]
    pub report_retry_interval: Duration,
}

fn executor_key_default() -> String {
    "jobx:executor".to_string()
}

fn executor_group_default() -> Option<String> {
    Some("default".to_string())
}

fn scan_interval_default() -> Duration {
    Duration::from_secs(1)
}

fn scan_block_default() -> Duration {
    Duration::from_secs(5)
}

fn max_messages_default() -> usize {
    100
}

fn report_retry_count_default() -> u32 {
    3
}

fn report_retry_interval_default() -> Duration {
    Duration::from_secs(1)
}