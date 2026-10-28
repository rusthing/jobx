//! # 过期任务清理配置
//!
//! `jobx_task` 是**只增不减**的表：一个 60s 间隔的计划每天长 1440 行、一年 50 万行。
//! 任务历史的价值随时间衰减——排查问题看的是最近几天，更早的记录只会拖慢任务列表
//! 分页、概览统计和 `jobx-task-reaper` 的巡检扫描。
//!
//! 本配置驱动的清理循环负责按**计划级**的保留时长（`jobx_job.task_expire_duration`）
//! 删除超期的任务记录：
//!
//! - 保留时长是**计划上的字段**，不是全局配置：健康检查这种高频计划可以只留 7 天，
//!   而人工触发、低频的审计型计划可以留一年甚至永久；
//! - 字段为 `NULL` 的计划**永不清理**（默认行为），升级后不会突然删掉历史数据。
//!
//! ## 与僵尸巡检（`jobx-task-reaper`）的分工
//!
//! 两者都在 `jobx_task` 上定时跑，但目的完全不同，不要混为一谈：
//!
//! - reaper 管**状态**：把卡在 `Running` 的记录收敛成 `Failed`，保留记录本身；
//! - cleaner 管**行数**：把超期的记录整行删掉，不关心它成功还是失败。
//!
//! 清理不区分任务状态（`Running` 的超期记录同样会被删）。这是有意的：卡住的记录由
//! reaper 在分钟级收敛，等它「超期」时早已不该是 `Running`；真被删掉的话，执行器
//! 事后上报只会命中 0 行，不影响执行器本身。

use arc_swap::ArcSwapOption;
use robotech::cfg::CfgError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use wheel_rs::serde::duration_serde;

/// 清理配置在 `changed` 映射里的键前缀。
///
/// 必须与 `AppConfig` 字段经 `#[serde(rename_all = "kebab-case")]` 变换后的顶层键名
/// 一致（`jobx_task_cleaner` → `jobx-task-cleaner`）。写成点号会让
/// `has_config_changed` 的前缀永不匹配，表现为**改了配置但清理仍用旧值**。
pub const JOBX_TASK_CLEANER_CONFIG_KEY: &str = "jobx-task-cleaner";

static JOBX_TASK_CLEANER_CONFIG: ArcSwapOption<JobxTaskCleanerConfig> =
    ArcSwapOption::const_empty();

pub fn get_jobx_task_cleaner_config() -> Result<Arc<JobxTaskCleanerConfig>, CfgError> {
    JOBX_TASK_CLEANER_CONFIG
        .load_full()
        .ok_or(CfgError::NotInit("Jobx task cleaner config not initialized".to_string()))
}

pub fn set_jobx_task_cleaner_config(config: &JobxTaskCleanerConfig) {
    JOBX_TASK_CLEANER_CONFIG.store(Some(Arc::new(config.clone())));
}

/// # 过期任务清理配置
///
/// 对应配置文件中的 `[jobx-task-cleaner]` 配置段。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct JobxTaskCleanerConfig {
    /// 是否启用清理
    ///
    /// 默认 `true`。关掉之后 `jobx_task` 会一直增长（例如你在别处有归档任务，
    /// 或者想先观察一段时间再动手）。
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// 清理周期
    ///
    /// 默认 10 分钟。清理是一条带 `limit` 的 DELETE，代价极低；调快没有收益，
    /// 调慢只会让超期记录的存活时间变长（而保留时长本身通常是「天」级的，
    /// 10 分钟的误差可以忽略）。
    #[serde(with = "duration_serde", default = "scan_interval_default")]
    pub scan_interval: Duration,
    /// 单批删除的行数
    ///
    /// 默认 1000。清理用 `delete from jobx_task where id in (select id ... limit N)`
    /// 分批执行，避免一次性删掉几十万行把 WAL 打爆、把表锁住（删除的行越多，
    /// 事务持有锁的时间越长，会挡住执行器写入新任务记录）。
    #[serde(default = "batch_size_default")]
    pub batch_size: u64,
    /// 单个计划每轮最多删几批
    ///
    /// 默认 20（即单计划每轮上限 `20 * batch_size = 2 万行`）。它给「首次给一个跑了
    /// 半年的计划打开保留时长」这种场景设了个刹车：积压不会一次性吃掉数据库 IO，
    /// 而是每 10 分钟推进一批，几小时清完。达到上限时会打一条 warn 说明还有剩余。
    #[serde(default = "max_batches_per_job_default")]
    pub max_batches_per_job: u32,
}

impl Default for JobxTaskCleanerConfig {
    fn default() -> Self {
        Self {
            enabled: enabled_default(),
            scan_interval: scan_interval_default(),
            batch_size: batch_size_default(),
            max_batches_per_job: max_batches_per_job_default(),
        }
    }
}

fn enabled_default() -> bool {
    true
}

fn scan_interval_default() -> Duration {
    Duration::from_mins(10)
}

fn batch_size_default() -> u64 {
    1000
}

fn max_batches_per_job_default() -> u32 {
    20
}
