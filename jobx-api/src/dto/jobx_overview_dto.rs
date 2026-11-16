use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// 总览统计（`GET /jobx/overview/summary`）
///
/// 把原先「前端拉最近 500 条计划 + 1200 条执行记录再自己聚合」的做法收到服务端，
/// 避免计划数 / 执行量超过采样上限后指标被低估。
///
/// ## 统计口径
///
/// - 24 小时窗口按 `assign_ms` 限定（分派时刻），不用 `exec_start_ms`；
/// - `running` 计入总量并单列，既不进成功也不进失败；
/// - `skipped` 单列，不进成功率分母；
/// - 成功率 = `success / (success + failed)`，没有已完成样本时为 `null`；
/// - 耗时分位只统计 `exec_start_ms`、`exec_end_ms` 都有的记录。
///
/// ## 数值表示
///
/// 这里的字段都是普通 JSON number（不是实体 VO 的字符串型 `U64`）：
/// 计数与毫秒时间戳都远小于 2^53，前端可以直接参与计算。
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxOverviewSummaryDto {
    /// 统计生成时刻（毫秒）
    pub generated_at_ms: u64,
    /// 24h 窗口起点（整点毫秒）：`hourly` 与执行器 `activity` 的第 0 个桶都从这里开始
    pub window_start_ms: u64,
    /// 计划维度
    pub jobs: JobxOverviewJobsDto,
    /// 执行维度（24h）
    pub tasks: JobxOverviewTasksDto,
    /// 按小时的成功/失败/跳过（固定 24 个桶，按时间升序）
    pub hourly: Vec<JobxOverviewHourDto>,
    /// 执行器维度（24h）
    pub executors: Vec<JobxOverviewExecutorDto>,
}

/// 计划维度统计（全量 `jobx_job`，与时间窗口无关）
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxOverviewJobsDto {
    /// 计划总数
    pub total: u64,
    /// 启用中
    pub enabled: u64,
    /// 已暂停
    pub disabled: u64,
    /// 手动分派（`Manual`）
    pub manual: u64,
    /// cron 表达式
    pub cron: u64,
    /// 固定延迟
    pub fixed_delay: u64,
    /// 固定频率
    pub fixed_rate: u64,
    /// 高频计划数（`high_freq = true`）
    pub high_freq: u64,
    /// 60 秒内即将分派的非手动计划数
    pub next_assign_soon: u64,
}

/// 执行维度统计（24h）
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxOverviewTasksDto {
    /// 24h 内分派的任务总数
    pub total: u64,
    /// 运行中（含已分派未开始）
    pub running: u64,
    /// 成功
    pub success: u64,
    /// 失败
    pub failed: u64,
    /// 跳过（一次都没执行）
    pub skipped: u64,
    /// 成功率（整数百分比，0-100）
    pub success_rate: Option<i32>,
    /// 平均耗时（毫秒），只统计 `exec_start_ms`、`exec_end_ms` 都有的记录
    pub avg_duration_ms: Option<u64>,
    /// 耗时 P50（毫秒）
    pub p50_duration_ms: Option<u64>,
    /// 耗时 P95（毫秒）
    pub p95_duration_ms: Option<u64>,
    /// 耗时 P99（毫秒）
    pub p99_duration_ms: Option<u64>,
}

/// 小时桶
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxOverviewHourDto {
    /// 桶起点（整点毫秒）
    pub hour_start_ms: u64,
    /// 成功
    pub success: u64,
    /// 失败
    pub failed: u64,
    /// 跳过
    pub skipped: u64,
}

/// 执行器维度统计
///
/// 执行器本身不落库，这里按计划与任务上的 `executor_code` 聚合。
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxOverviewExecutorDto {
    /// 执行器编码
    pub code: String,
    /// 绑定到该编码的计划总数
    pub jobs: u64,
    /// 其中启用中的计划数
    pub enabled_jobs: u64,
    /// 其中高频计划数
    pub high_freq_jobs: u64,
    /// 24h 内执行次数
    pub tasks_24h: u64,
    /// 运行中
    pub running: u64,
    /// 成功
    pub success_24h: u64,
    /// 失败
    pub failed_24h: u64,
    /// 跳过
    pub skipped_24h: u64,
    /// 成功率（整数百分比，0-100）
    pub success_rate: Option<i32>,
    /// 平均耗时（毫秒）
    pub avg_duration_ms: Option<u64>,
    /// 最后活动时刻（毫秒）
    pub last_activity_ms: Option<u64>,
    /// 24h 活跃度（固定 24 个桶，与 summary.hourly 对齐）
    pub activity: Vec<JobxOverviewActivityDto>,
}

/// 执行器小时活跃度
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxOverviewActivityDto {
    /// 该小时执行次数
    pub count: u64,
    /// 其中失败次数
    pub failed: u64,
}
