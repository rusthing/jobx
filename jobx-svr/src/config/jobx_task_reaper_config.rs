//! # 僵尸任务巡检配置
//!
//! 任务记录的生命周期里有一段「服务端已经建单、执行器还没上报结果」的窗口。
//! 正常情况下这段窗口只有秒级到分钟级，但执行器进程崩溃、被滚动发布杀掉、或者
//! 上报请求失败时，记录会**永远**停在 `status = Running`：
//!
//! - 消息在 `take` 成功那一刻就 ACK 了，Redis PEL 里没有残留，`XAUTOCLAIM` 救不到；
//! - `jobx_task` 行没有任何「最后更新时间」的超时判定，服务端也不会主动收尾。
//!
//! 结果是任务列表里堆积一堆「运行中 / 耗时待执行」的记录，概览页的「正在执行」
//! 长期虚高。本配置驱动的巡检循环负责把这些记录收敛成 `Failed`。
//!
//! ## 为什么必须带阈值
//!
//! 巡检只看「时间」这一个信号，它分不清「进程死了」和「任务本来就慢」。所以三个
//! 阈值都要留足余量，宁可晚回收，也不要误杀正在跑的任务：
//!
//! - 被误判为超时的任务，执行器跑完后仍会正常上报，状态会被改回真实结果
//!   （后写覆盖），因此误判是可自愈的，但会在历史里留下一条假的 `Failed` 明细；
//! - 真僵尸不回收，就一直挂在列表里，没有任何自愈可能。

use arc_swap::ArcSwapOption;
use robotech::cfg::CfgError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use wheel_rs::serde::duration_serde;

/// 巡检配置在 `changed` 映射里的键前缀。
///
/// 必须与 `AppConfig` 字段经 `#[serde(rename_all = "kebab-case")]` 变换后的顶层键名
/// 一致（`jobx_task_reaper` → `jobx-task-reaper`）。写成点号会让
/// `has_config_changed` 的前缀永不匹配，表现为**改了配置但巡检仍用旧值**。
pub const JOBX_TASK_REAPER_CONFIG_KEY: &str = "jobx-task-reaper";

static JOBX_TASK_REAPER_CONFIG: ArcSwapOption<JobxTaskReaperConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_task_reaper_config() -> Result<Arc<JobxTaskReaperConfig>, CfgError> {
    JOBX_TASK_REAPER_CONFIG
        .load_full()
        .ok_or(CfgError::NotInit("Jobx task reaper config not initialized".to_string()))
}

pub fn set_jobx_task_reaper_config(config: &JobxTaskReaperConfig) {
    JOBX_TASK_REAPER_CONFIG.store(Some(Arc::new(config.clone())));
}

/// # 僵尸任务巡检配置
///
/// 对应配置文件中的 `[jobx-task-reaper]` 配置段。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct JobxTaskReaperConfig {
    /// 是否启用巡检
    ///
    /// 默认 `true`。关掉之后僵尸记录会一直留在库里（例如你在别处有更完善的
    /// 对账机制，或者想先手工清理历史数据）。
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// 巡检周期
    ///
    /// 默认 60s，与调度器的 `scan_interval` 同量级。巡检是一条带条件的 UPDATE，
    /// 代价极低，调快没有收益；调慢只会让僵尸记录的存活时间变长。
    #[serde(with = "duration_serde", default = "scan_interval_default")]
    pub scan_interval: Duration,
    /// 「已分派但未开始执行」的宽限期
    ///
    /// 默认 5 分钟。执行器 `take` 之后要等 `scheduled_exec_start_ms` 才真正开始
    /// （等待时长 = 调度器发布提前量 + `assign_lead_ms`，默认最多约 2 分钟），
    /// 再加上上报的 HTTP 往返，5 分钟是很宽松的余量。
    ///
    /// 只有同时满足 `status = Running`、`exec_start_ms is null`、
    /// `scheduled_exec_start_ms < now - 宽限期` 的任务才会被回收。
    #[serde(with = "duration_serde", default = "start_grace_default")]
    pub start_grace: Duration,
    /// 「已开始执行但未结束」的超时阈值（普通任务）
    ///
    /// 默认 30 分钟。超过这个时长还没上报结果，视为执行器已失联。
    /// 按你的业务最长执行时间调整：把它设成 **P99 执行时长的数倍**。
    #[serde(with = "duration_serde", default = "exec_timeout_default")]
    pub exec_timeout: Duration,
    /// 「已开始执行但未结束」的超时阈值（高频任务）
    ///
    /// 默认 6 小时，单独一档。高频任务的设计就是「一次分派、连续跑一段时间」，
    /// 用 `exec_timeout` 那档会把它全部误杀。
    ///
    /// 正常情况下高频循环跑到 `jobx_task.exec_deadline_ms`（= 预定开始执行时刻 +
    /// `high_freq_assign_interval`，默认 5 分钟）就收尾上报，根本到不了这个阈值。
    /// 所以这一档只是**兜底中的兜底**：捞那些执行器侧循环彻底失控（例如升级前创建的
    /// 老记录没有 `exec_deadline_ms`、`valid_end_ms` 也为空 → `end_ms = u64::MAX`）
    /// 而永远不结束的任务。
    #[serde(with = "duration_serde", default = "high_freq_exec_timeout_default")]
    pub high_freq_exec_timeout: Duration,
}

impl Default for JobxTaskReaperConfig {
    fn default() -> Self {
        Self {
            enabled: enabled_default(),
            scan_interval: scan_interval_default(),
            start_grace: start_grace_default(),
            exec_timeout: exec_timeout_default(),
            high_freq_exec_timeout: high_freq_exec_timeout_default(),
        }
    }
}

fn enabled_default() -> bool {
    true
}

fn scan_interval_default() -> Duration {
    Duration::from_secs(60)
}

fn start_grace_default() -> Duration {
    Duration::from_mins(5)
}

fn exec_timeout_default() -> Duration {
    Duration::from_mins(30)
}

fn high_freq_exec_timeout_default() -> Duration {
    Duration::from_hours(6)
}
