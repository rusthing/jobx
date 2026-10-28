//! # 过期任务清理
//!
//! 周期性删除「超过计划保留时长」的 `jobx_task` 记录。
//!
//! ## 为什么需要它
//!
//! `jobx_task` 只增不减：一个 60s 间隔的计划每天长 1440 行。这些记录一旦超期就只剩
//! 负收益 —— 拖慢任务列表分页、拖慢概览统计，还让 `jobx-task-reaper` 的部分索引
//! 越扫越大。保留多久属于**业务判断**，所以它做成了计划上的字段
//! （`jobx_job.task_expire_duration`）而不是全局配置：健康检查留 7 天，审计型计划
//! 可以留一年，不关心历史的计划留空（`NULL`）永不清理。
//!
//! ## 与 `jobx-task-reaper` 的分工
//!
//! reaper 管**状态**（卡在 Running 的收敛成 Failed，记录留着），cleaner 管**行数**
//! （超期的整行删掉，不看状态）。两者的扫描周期都是分钟级，互不依赖：
//! 即使 cleaner 关掉，reaper 也照常工作；反之亦然。
//!
//! 具体判定与分批策略见 [`JobxTaskSvc::clean_expired_tasks`] 与
//! [`JobxTaskCleanerConfig`]。

use crate::config::{
    JOBX_TASK_CLEANER_CONFIG_KEY, JobxTaskCleanerConfig, get_jobx_task_cleaner_config,
    set_jobx_task_cleaner_config,
};
use crate::svc::JobxTaskSvc;
use config::Value;
use sea_orm::DatabaseTransaction;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn};
use wheel_rs::config_utils::has_config_changed;
use wheel_rs::time_utils::{build_ticker, now_ms};

/// 配置的默认清理周期（配置里把间隔写成 0 时用它兜底）
const FALLBACK_SCAN_INTERVAL: Duration = Duration::from_mins(10);

/// # 初始化过期任务清理
///
/// 与 `setup_jobx_task_reaper` 保持同一套模式：
/// - 每次配置加载/热更新都会刷新全局配置；
/// - **只在首次初始化**（`changed` 为 `None`）时启动清理循环，热更新不会重复起线程。
///
/// ## 参数
/// * `config` - 清理配置
/// * `changed` - 配置中心回调给出的「本次变更的配置段」，首次启动时为 `None`
pub fn setup_jobx_task_cleaner(
    config: JobxTaskCleanerConfig,
    changed: &Option<HashMap<String, Value>>,
) {
    info!("setup jobx task cleaner config...: {config:?}");
    if changed
        .as_ref()
        .map(|changed| has_config_changed(JOBX_TASK_CLEANER_CONFIG_KEY, changed))
        .unwrap_or(true)
    {
        set_jobx_task_cleaner_config(&config);
        if changed.is_none() {
            tokio::spawn(run_task_cleaner_loop(config.scan_interval));
        }
    }
}

/// # 清理循环
///
/// 每 `scan_interval` 执行一轮，运行期间动态读取最新配置（热更新后自动生效）。
/// 该函数在进程退出前不会返回，需通过 `tokio::spawn` 在独立任务中运行。
///
/// 单轮失败只记日志不退出：清理是维护性工作，一次 DB 抖动不该让循环永久停摆
/// （停摆的后果是表继续长大，而不是任务丢失）。
async fn run_task_cleaner_loop(initial_scan_interval: Duration) {
    let scan_interval = if initial_scan_interval.is_zero() {
        warn!("过期任务清理的扫描间隔为0，使用默认值 {FALLBACK_SCAN_INTERVAL:?}");
        FALLBACK_SCAN_INTERVAL
    } else {
        initial_scan_interval
    };
    let mut ticker = build_ticker(scan_interval);
    loop {
        ticker.tick().await;
        let config = match get_jobx_task_cleaner_config() {
            Ok(config) => config,
            Err(e) => {
                warn!("获取过期任务清理配置失败: {e:?}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        if !config.enabled {
            continue;
        }

        let now = now_ms() as i64;
        match JobxTaskSvc::clean_expired_tasks::<DatabaseTransaction>(
            now,
            config.batch_size,
            config.max_batches_per_job,
            None,
        )
        .await
        {
            // 健康状态下每轮都是 0 条，只在真删到东西时打日志，否则全是噪声
            Ok(result) if result.total() > 0 => {
                info!(
                    "清理过期任务 {} 条（涉及 {} 个计划）",
                    result.deleted, result.jobs
                );
                if result.truncated > 0 {
                    warn!(
                        "有 {} 个计划的过期任务未清完（已达单轮批次数上限 {}），\
                         下一轮继续；积压较多可调大 jobx-task-cleaner.max-batches-per-job",
                        result.truncated, config.max_batches_per_job
                    );
                }
            }
            Ok(_) => {}
            Err(e) => warn!("过期任务清理失败: {e:?}"),
        }

        if config.scan_interval != ticker.period() && !config.scan_interval.is_zero() {
            ticker = build_ticker(config.scan_interval);
        }
    }
}
