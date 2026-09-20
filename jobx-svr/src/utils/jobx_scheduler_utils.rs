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
    info!("setup scheduler config...: {schedule_config:?}");
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
    let mut ticker = build_ticker(initial_scan_interval);
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
        if config.scan_interval != ticker.period() {
            ticker = build_ticker(config.scan_interval);
        }
    }
}
