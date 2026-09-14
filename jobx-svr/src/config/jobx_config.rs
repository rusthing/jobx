use crate::config::scheduler_config::SchedulerConfig;
use crate::config::setup_jobx_scheduler_config;
use config::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn setup_jobx_config(jobx_config: JobxConfig, changed: &Option<HashMap<String, Value>>) {
    setup_jobx_scheduler_config(jobx_config.scheduler, changed);
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct JobxConfig {
    /// 调度器配置
    pub scheduler: SchedulerConfig,
}
