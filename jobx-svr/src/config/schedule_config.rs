use arc_swap::ArcSwapOption;
use config::Value;
use robotech::cfg::CfgError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use wheel_rs::config_utils::has_config_changed;

const JOBX_SCHEDULE_CONFIG_KEY: &str = "schedule";
static JOBX_SCHEDULE_CONFIG: ArcSwapOption<ScheduleConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_schedule_config() -> Result<Arc<ScheduleConfig>, CfgError> {
    JOBX_SCHEDULE_CONFIG.load_full().ok_or(CfgError::NotInit(
        "Schedule config not initialized".to_string(),
    ))
}

pub fn setup_jobx_schedule_config(
    schedule_config: ScheduleConfig,
    changed: &Option<HashMap<String, Value>>,
) {
    info!("setup schedule config...: {schedule_config:?}");
    if changed
        .as_ref()
        .map(|changed| has_config_changed(JOBX_SCHEDULE_CONFIG_KEY, changed))
        .unwrap_or(true)
    {


        JOBX_SCHEDULE_CONFIG.store(Some(Arc::new(schedule_config)));
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ScheduleConfig {}
