use arc_swap::ArcSwapOption;
use robotech::cfg::CfgError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use wheel_rs::serde::duration_serde;

pub const JOBX_SCHEDULER_CONFIG_KEY: &str = "jobx.scheduler";
static JOBX_SCHEDULER_CONFIG: ArcSwapOption<JobxSchedulerConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_scheduler_config() -> Result<Arc<JobxSchedulerConfig>, CfgError> {
    JOBX_SCHEDULER_CONFIG.load_full().ok_or(CfgError::NotInit(
        "Scheduler config not initialized".to_string(),
    ))
}

pub fn set_jobx_scheduler_config(config: JobxSchedulerConfig) {
    JOBX_SCHEDULER_CONFIG.store(Some(Arc::new(config.clone())));
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct JobxSchedulerConfig {
    /// Redis Stream 的 key的前缀，后面跟着任务执行器的编码
    #[serde(default = "executor_key_default")]
    pub executor_key: String,
    /// 扫描间隔
    #[serde(with = "duration_serde", default = "scan_interval_default")]
    pub scan_interval: Duration,
    /// 查询开始时间比当前早多少时间
    #[serde(default = "query_start_earlier_duration_default")]
    pub query_start_earlier_duration: Duration,
    /// 查询结束时间比当前晚多少时间
    #[serde(default = "query_end_later_duration_default")]
    pub query_end_later_duration: Duration,
    /// 分配任务提前多少时间
    #[serde(with = "duration_serde", default = "assign_lead_duration_default")]
    pub assign_lead_duration: Duration,
    /// 高频任务时长阈值，当任务的执行间隔小于此阈值时，标记为高频任务
    #[serde(with = "duration_serde", default = "high_freq_threshold_default")]
    pub high_freq_threshold_duration: Duration,
    /// 高频任务分派间隔，高频任务每次分派后，下次分派时间 = 上次分派时间 + 此间隔
    #[serde(with = "duration_serde", default = "high_freq_assign_interval_default")]
    pub high_freq_assign_interval: Duration,
    /// 发布任务到 Redis Stream 的最大重试次数
    #[serde(default = "publish_max_retries_default")]
    pub publish_max_retries: u32,
    /// 发布任务到 Redis Stream 失败时的重试间隔
    #[serde(with = "duration_serde", default = "publish_retry_interval_default")]
    pub publish_retry_interval: Duration,
}

fn executor_key_default() -> String {
    "jobx:executor".to_string()
}

fn scan_interval_default() -> Duration {
    Duration::from_secs(60)
}

fn query_start_earlier_duration_default() -> Duration {
    Duration::from_secs(60)
}

fn query_end_later_duration_default() -> Duration {
    Duration::from_secs(60)
}

fn assign_lead_duration_default() -> Duration {
    Duration::from_secs(60)
}

fn high_freq_threshold_default() -> Duration {
    Duration::from_secs(60)
}

fn high_freq_assign_interval_default() -> Duration {
    Duration::from_mins(5)
}

fn publish_max_retries_default() -> u32 {
    5
}

fn publish_retry_interval_default() -> Duration {
    Duration::from_secs(8)
}
