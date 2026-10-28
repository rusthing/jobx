//! # 僵尸任务巡检
//!
//! 周期性把「服务端已建单、执行器再也不会来上报」的 `jobx_task` 记录收敛成 `Failed`。
//!
//! ## 为什么需要它
//!
//! 任务记录的生命周期是：调度器发布 → 执行器 `take`（落一条 `status = Running` 的记录，
//! 此时 `exec_start_ms` 还是空）→ 等到预定执行时刻 → 写 `exec_start_ms` → 执行
//! → 上报结果。窗口内的任何一次进程退出（滚动发布、OOM、被 kill）都会让记录停在
//! 「运行中」：
//!
//! - 消息在 `take` 成功那一刻已经 ACK，Redis PEL 里没有残留，`XAUTOCLAIM` 救不到；
//! - `jobx_task` 行此前没有任何超时判定，服务端也不会主动收尾。
//!
//! 于是列表里越积越多「运行中 / 耗时待执行」，概览页的「正在执行」长期虚高。
//!
//! ## 与执行器侧的关系
//!
//! 执行器（`jobx-wkr`）在优雅停机时会主动把在途任务关闭，这是**第一道**防线，
//! 秒级生效。但 SIGKILL、OOM、机器掉电这些场景下执行器来不及收尾，所以服务端
//! 仍然需要这道**兜底**：按时间判定，慢一点但一定收得住。
//!
//! 具体规则与阈值见 [`JobxTaskSvc::reap_zombie_tasks`] 与 [`JobxTaskReaperConfig`]。

use crate::config::{
    JOBX_TASK_REAPER_CONFIG_KEY, JobxTaskReaperConfig, get_jobx_task_reaper_config,
    set_jobx_task_reaper_config,
};
use crate::svc::JobxTaskSvc;
use config::Value;
use sea_orm::DatabaseTransaction;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::time_utils::{build_ticker, now_ms};

/// 配置的默认巡检周期（配置里把间隔写成 0 时用它兜底）
const FALLBACK_SCAN_INTERVAL: Duration = Duration::from_secs(60);

/// # 初始化僵尸任务巡检
///
/// 与调度器的 `setup_jobx_scheduler` 保持同一套模式：
/// - 每次配置加载/热更新都会刷新全局配置；
/// - **只在首次初始化**（`changed` 为 `None`）时启动巡检循环，热更新不会重复起线程。
///
/// ## 参数
/// * `config` - 巡检配置
/// * `changed` - 配置中心回调给出的「本次变更的配置段」，首次启动时为 `None`
pub fn setup_jobx_task_reaper(
    config: JobxTaskReaperConfig,
    changed: &Option<HashMap<String, Value>>,
) {
    info!("setup jobx task reaper config...: {config:?}");
    if changed
        .as_ref()
        .map(|changed| has_config_changed(JOBX_TASK_REAPER_CONFIG_KEY, changed))
        .unwrap_or(true)
    {
        set_jobx_task_reaper_config(&config);
        if changed.is_none() {
            tokio::spawn(run_task_reaper_loop(config.scan_interval));
        }
    }
}

/// # 巡检循环
///
/// 每 `scan_interval` 执行一轮，运行期间动态读取最新配置（热更新后自动生效）。
/// 该函数在进程退出前不会返回，需通过 `tokio::spawn` 在独立任务中运行。
///
/// 单轮失败只记日志不退出：巡检是兜底手段，一次 DB 抖动不该让兜底永久失效。
async fn run_task_reaper_loop(initial_scan_interval: Duration) {
    let scan_interval = if initial_scan_interval.is_zero() {
        warn!("僵尸任务巡检的扫描间隔为0，使用默认值 {FALLBACK_SCAN_INTERVAL:?}");
        FALLBACK_SCAN_INTERVAL
    } else {
        initial_scan_interval
    };
    let mut ticker = build_ticker(scan_interval);
    loop {
        ticker.tick().await;
        let config = match get_jobx_task_reaper_config() {
            Ok(config) => config,
            Err(e) => {
                warn!("获取僵尸任务巡检配置失败: {e:?}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        if !config.enabled {
            continue;
        }

        let now = now_ms() as i64;
        match JobxTaskSvc::reap_zombie_tasks::<DatabaseTransaction>(
            now,
            config.start_grace.as_millis() as i64,
            config.exec_timeout.as_millis() as i64,
            config.high_freq_exec_timeout.as_millis() as i64,
            None,
        )
        .await
        {
            // 只在真的收到东西时打 info：健康状态下每轮都是 0 条，否则日志全是噪声
            Ok(result) if result.total() > 0 => warn!(
                "回收僵尸任务 {} 条（未开始 {} / 执行超时 {} / 高频超时 {}）",
                result.total(),
                result.not_started,
                result.timed_out,
                result.high_freq_timed_out,
            ),
            Ok(_) => {}
            Err(e) => warn!("僵尸任务巡检失败: {e:?}"),
        }

        if config.scan_interval != ticker.period() && !config.scan_interval.is_zero() {
            ticker = build_ticker(config.scan_interval);
        }
    }
}
