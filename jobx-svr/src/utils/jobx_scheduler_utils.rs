use crate::config::JobxSchedulerConfig;
use crate::svc::JobxJobSvc;
use arc_swap::ArcSwapOption;
use config::Value;
use robotech::cfg::CfgError;
use sea_orm::DatabaseTransaction;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;

const JOBX_SCHEDULER_CONFIG_KEY: &str = "jobx.scheduler";
static JOBX_SCHEDULER_CONFIG: ArcSwapOption<JobxSchedulerConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_scheduler_config() -> Result<Arc<JobxSchedulerConfig>, CfgError> {
    JOBX_SCHEDULER_CONFIG.load_full().ok_or(CfgError::NotInit(
        "Scheduler config not initialized".to_string(),
    ))
}

pub fn setup_jobx_scheduler_config(
    schedule_config: JobxSchedulerConfig,
    changed: &Option<HashMap<String, Value>>,
) {
    info!("setup scheduler config...: {schedule_config:?}");
    if changed
        .as_ref()
        .map(|changed| has_config_changed(JOBX_SCHEDULER_CONFIG_KEY, changed))
        .unwrap_or(true)
    {
        JOBX_SCHEDULER_CONFIG.store(Some(Arc::new(schedule_config.clone())));

        // 如果是首次配置，启动扫描线程
        if changed.is_none() {
            tokio::spawn(async move {
                let mut ticker = build_ticker(schedule_config.scan_interval);
                loop {
                    ticker.tick().await;
                    let config = match get_jobx_scheduler_config() {
                        Ok(c) => c,
                        Err(e) => {
                            warn!("获取调度配置失败: {e:?}");
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    };
                    if let Err(e) =
                        JobxJobSvc::scan_and_publish::<DatabaseTransaction>(&config, None).await
                    {
                        warn!("调度器扫描异常: {e:?}");
                    }
                    if config.scan_interval != ticker.period() {
                        ticker = build_ticker(config.scan_interval);
                    }
                }
            });
        }
    }
}

fn build_ticker(period: Duration) -> tokio::time::Interval {
    let mut ticker = interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}
