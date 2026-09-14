use crate::config::schedule_config::ScheduleConfig;
use crate::config::setup_jobx_schedule_config;
use config::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn setup_jobx_config(jobx_config: JobxConfig, changed: &Option<HashMap<String, Value>>) {
    setup_jobx_schedule_config(jobx_config.schedule, changed);
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct JobxConfig {
    /// 调度器配置
    #[serde(default)]
    pub schedule: ScheduleConfig,
}
