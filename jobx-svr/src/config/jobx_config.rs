use arc_swap::ArcSwapOption;
use config::Value;
use robotech::cfg::CfgError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use wheel_rs::config_utils::has_config_changed;

const JOBX_CONFIG_KEY: &str = "jobx";
static JOBX_CONFIG: ArcSwapOption<JobxConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_config() -> Result<Arc<JobxConfig>, CfgError> {
    JOBX_CONFIG
        .load_full()
        .ok_or(CfgError::NotInit("JobX config not initialized".to_string()))
}

pub fn setup_jobx_config(jobx_config: JobxConfig, changed: &Option<HashMap<String, Value>>) {
    info!("setup jobx config...: {jobx_config:?}");
    if changed
        .as_ref()
        .map(|changed| has_config_changed(JOBX_CONFIG_KEY, changed))
        .unwrap_or(true)
    {
        JOBX_CONFIG.store(Some(Arc::new(jobx_config)));
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct JobxConfig {
    /// 任务调度器线程数
    #[serde(default = "scheduler_workers_default")]
    pub scheduler_workers: usize,
    /// 任务执行超时时间(秒)
    #[serde(default = "task_timeout_seconds_default")]
    pub task_timeout_seconds: u64,
    /// 最大重试次数
    #[serde(default = "max_retry_count_default")]
    pub max_retry_count: u32,
}

impl Default for JobxConfig {
    fn default() -> Self {
        Self {
            scheduler_workers: scheduler_workers_default(),
            task_timeout_seconds: task_timeout_seconds_default(),
            max_retry_count: max_retry_count_default(),
        }
    }
}

fn scheduler_workers_default() -> usize {
    4
}

fn task_timeout_seconds_default() -> u64 {
    3600
}

fn max_retry_count_default() -> u32 {
    3
}
