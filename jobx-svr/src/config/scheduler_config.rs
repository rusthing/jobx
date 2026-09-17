use crate::svc::JobxJobSvc;
use arc_swap::ArcSwapOption;
use config::Value;
use robotech::cfg::CfgError;
use sea_orm::DatabaseTransaction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::serde::duration_serde;

const JOBX_SCHEDULER_CONFIG_KEY: &str = "jobx.scheduler";
static JOBX_SCHEDULER_CONFIG: ArcSwapOption<SchedulerConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_scheduler_config() -> Result<Arc<SchedulerConfig>, CfgError> {
    JOBX_SCHEDULER_CONFIG.load_full().ok_or(CfgError::NotInit(
        "Scheduler config not initialized".to_string(),
    ))
}

pub fn setup_jobx_scheduler_config(
    schedule_config: SchedulerConfig,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchedulerConfig {
    /// Redis Stream 的 key
    #[serde(default = "stream_key_default")]
    pub stream_key: String,
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
}

fn build_ticker(period: Duration) -> tokio::time::Interval {
    let mut ticker = interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}

fn stream_key_default() -> String {
    "jobx:scheduler".to_string()
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
