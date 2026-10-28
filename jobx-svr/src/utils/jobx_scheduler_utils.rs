use crate::config::{
    get_jobx_scheduler_config, set_jobx_scheduler_config, JobxSchedulerConfig,
    JOBX_SCHEDULER_CONFIG_KEY,
};
use crate::svc::JobxJobSvc;
use config::Value;
use sea_orm::DatabaseTransaction;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::time_utils::build_ticker;

pub fn setup_jobx_scheduler(
    schedule_config: JobxSchedulerConfig,
    changed: &Option<HashMap<String, Value>>,
) {
    info!("setup jobx scheduler config...: {schedule_config:?}");
    // 滞后兜底的阈值必须小于查询窗口的下界，否则「滞后超阈值」的计划
    // 早就落到窗口外了，两条规则互相矛盾，兜底永远不触发。
    if schedule_config.max_assign_lag_duration >= schedule_config.query_start_earlier_duration {
        warn!(
            "max-assign-lag-duration({:?}) 不小于 query-start-earlier-duration({:?})，\
             滞后兜底将永远不会触发",
            schedule_config.max_assign_lag_duration, schedule_config.query_start_earlier_duration
        );
    }
    if changed
        .as_ref()
        .map(|changed| has_config_changed(JOBX_SCHEDULER_CONFIG_KEY, changed))
        .unwrap_or(true)
    {
        set_jobx_scheduler_config(schedule_config.clone());

        // 如果是首次配置，启动扫描线程
        if changed.is_none() {
            tokio::spawn(run_scheduler_loop(schedule_config.scan_interval));
        }
    }
}

/// # 启动调度器扫描循环
///
/// 按配置的扫描间隔定时拉取可发布任务并推送到 Redis Stream。
/// 运行期间会动态读取最新配置，若扫描间隔变更则自动调整 ticker。
/// 该函数不会返回，需通过 `tokio::spawn` 在独立任务中运行。
async fn run_scheduler_loop(initial_scan_interval: Duration) {
    let scan_interval = if initial_scan_interval.is_zero() {
        warn!("初始扫描间隔为0，使用默认值60s");
        Duration::from_secs(60)
    } else {
        initial_scan_interval
    };
    let mut ticker = build_ticker(scan_interval);
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
        if let Err(e) = JobxJobSvc::scan_and_publish::<DatabaseTransaction>(&config, None).await {
            warn!("调度器扫描异常: {e:?}");
        }
        if config.scan_interval != ticker.period() && !config.scan_interval.is_zero() {
            ticker = build_ticker(config.scan_interval);
        } else if config.scan_interval.is_zero() {
            warn!("配置中的扫描间隔为0，保持当前间隔不变");
        }
    }
}