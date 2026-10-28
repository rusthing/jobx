use arc_swap::ArcSwapOption;
use robotech::cfg::CfgError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use wheel_rs::serde::duration_serde;

/// 调度配置在 `changed` 映射里的键前缀。
///
/// ⚠️ 必须与 [`AppConfig`] 字段经 `#[serde(rename_all = "kebab-case")]` 变换后的
/// **顶层键名**完全一致（`jobx_scheduler` → `jobx-scheduler`），不能写成点号分隔的
/// `jobx.scheduler` —— 配置中心回调给的 `changed` 是按 TOML 顶层表名做键的
/// （实测 `Some({"jobx-scheduler": Table({...})})`），而 `has_config_changed` 用的是
/// `key.starts_with(前缀)`。写成点号会让前缀永不匹配，表现为
/// **配置文件改了、日志也打印了新值，但调度器仍在用旧配置跑**（ticker 不重建）。
pub const JOBX_SCHEDULER_CONFIG_KEY: &str = "jobx-scheduler";
static JOBX_SCHEDULER_CONFIG: ArcSwapOption<JobxSchedulerConfig> = ArcSwapOption::const_empty();

pub fn get_jobx_scheduler_config() -> Result<Arc<JobxSchedulerConfig>, CfgError> {
    JOBX_SCHEDULER_CONFIG.load_full().ok_or(CfgError::NotInit(
        "Scheduler config not initialized".to_string(),
    ))
}

pub fn set_jobx_scheduler_config(config: JobxSchedulerConfig) {
    JOBX_SCHEDULER_CONFIG.store(Some(Arc::new(config.clone())));
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct JobxSchedulerConfig {
    /// Redis Stream 的 key的前缀，后面跟着任务执行器的编码
    #[serde(default = "executor_key_default")]
    pub executor_key: String,
    /// 扫描间隔
    #[serde(with = "duration_serde", default = "scan_interval_default")]
    pub scan_interval: Duration,
    /// 查询开始时间比当前早多少时间
    #[serde(default = "query_start_earlier_duration_default")]
    pub query_start_earlier_duration: Duration,
    /// 查询结束时间比当前晚多少时间
    #[serde(default = "query_end_later_duration_default")]
    pub query_end_later_duration: Duration,
    /// 允许的最大分派滞后
    ///
    /// `next_assign_ms` 落后 `now` 超过这个值，说明逻辑时间轴已经失去意义：长时间停机、
    /// worker 长期掉线、或者积压到追赶速率追不上。调度器每轮扫描前会把这类计划直接
    /// 拉回 `now`（丢弃错过的拍子，从现在重新开始）并打一条 warn。
    ///
    /// ## 为什么必须有这一档
    ///
    /// 发布窗口是 `[now - query_start_earlier_duration, now + query_end_later_duration]`，
    /// 而 `next_assign_ms` 的推进是**开环**的（起点取旧值、不对齐墙钟）。一旦滞后超过
    /// `query_start_earlier_duration`（默认 3 天），计划就落不进窗口，**永远不会再被发布**
    /// —— 静默停摆，日志里一条记录都没有。这一档把「停摆」变成「恢复 + 告警」。
    ///
    /// ## 默认 1 小时
    ///
    /// 必须小于 `query_start_earlier_duration`，否则永远不会触发（启动时会 warn）。
    /// 也不要调得太小：它同时决定「停机多久之内还保留补拍语义」，超过这个值之后
    /// 错过的拍子会被丢弃，只补当前这一拍。
    #[serde(with = "duration_serde", default = "max_assign_lag_duration_default")]
    pub max_assign_lag_duration: Duration,
    /// 分配任务提前多少时间
    #[serde(with = "duration_serde", default = "assign_lead_duration_default")]
    pub assign_lead_duration: Duration,
    /// 高频任务时长阈值，当任务的执行间隔小于此阈值时，标记为高频任务
    #[serde(with = "duration_serde", default = "high_freq_threshold_default")]
    pub high_freq_threshold_duration: Duration,
    /// 高频任务分派间隔，高频任务每次分派后，下次分派时间 = 上次分派时间 + 此间隔
    #[serde(with = "duration_serde", default = "high_freq_assign_interval_default")]
    pub high_freq_assign_interval: Duration,
    /// 发布任务到 Redis Stream 的最大重试次数
    #[serde(default = "publish_max_retries_default")]
    pub publish_max_retries: u32,
    /// 发布任务到 Redis Stream 失败时的重试间隔
    #[serde(with = "duration_serde", default = "publish_retry_interval_default")]
    pub publish_retry_interval: Duration,
    /// Redis Stream 保留的最大消息条数（近似上限）
    ///
    /// 这是**全局默认值**：`jobx_job.stream_max_len` 不为空的计划会用自己的值覆盖它，
    /// 只有该列为空的计划才落回这里（见 `JobxJobSvc::scan_and_publish`）。
    ///
    /// 发布时通过 `XADD ... MAXLEN ~ <该值>` 顺带裁剪，防止长期运行后 Stream 无限增长。
    ///
    /// ## Stream 在这个系统里是什么
    ///
    /// 它是"调度器 → worker"的**临时交接缓冲**，不是审计日志，也不是可靠队列：
    /// 消息被消费并 ACK 之后就没有价值了，稳态下理想长度是 **0~1**。
    ///
    /// - 发布侧：`scan_interval` 默认 60s，每轮最多发布「本轮到期任务数」条；
    /// - 消费侧：worker `scan_interval` 默认 1s，单次最多 `max_messages` 条。
    ///
    /// 长度只在 worker 掉线时才会涨，涨起来的绝大多数是重复消息（`next_assign_ms`
    /// 没被推进，调度器每 60s 重发一次同一个 job）。**所以这个值够用就行，不必很大。**
    ///
    /// ## 默认 10000
    ///
    /// 这是一个**很宽松的兜底值**，目的是"宁可占点内存，也不要在故障恢复时把还没消费的
    /// 消息裁掉"。按单条消息 ≈ 1.5KB 估算，满载约 15MB，属于可接受量级。
    ///
    /// 真正需要收紧的场景请用**计划级覆盖**：把 `jobx_job.stream_max_len` 设小。
    /// 典型例子是本仓 dbz 的健康检查计划（`DbzConfig::HealthCheckConfig` 里默认给 100）：
    /// 它每 60s 必发一条、且只需要"当前立刻执行一次"，Stream 上留一小段历史纯属噪声。
    ///
    /// ## `~` 模式的实际地板是 100
    ///
    /// `MAXLEN ~` 按 macro node 整块裁剪，每个 node 默认装
    /// `stream-node-max-entries = 100` 条；Redis 只在「删掉最早那个 node 之后长度仍 ≥ maxlen」
    /// 时才删。因此**任何 ≤ 100 的 maxlen，实际长度都会稳定在 100~199 之间** ——
    /// 写 10 和写 100 效果一样。所以想要"短 Stream"时，设 100 就到头了，再往下没有意义。
    ///
    /// ## 裁掉了会丢任务吗
    ///
    /// 不会。裁剪删的是**最早**的条目，不区分是否已消费；被裁掉的未消费消息对应的
    /// `jobx_job.next_assign_ms` 没有被推进（推进只发生在 worker 成功 `take` 时），
    /// 因此 `list_publishable` 的 `NOT EXISTS(jobx_task...)` 条件依然成立，
    /// 调度器下一轮扫描会重新发布。所以这个值**只影响内存与恢复噪声，不影响正确性**
    /// （前提是 worker 掉线时长不超过 `query_start_earlier_duration`，默认 3 天）。
    #[serde(default = "stream_max_len_default")]
    pub stream_max_len: usize,
}

impl Default for JobxSchedulerConfig {
    fn default() -> Self {
        Self {
            executor_key: executor_key_default(),
            scan_interval: scan_interval_default(),
            query_start_earlier_duration: query_start_earlier_duration_default(),
            query_end_later_duration: query_end_later_duration_default(),
            max_assign_lag_duration: max_assign_lag_duration_default(),
            assign_lead_duration: assign_lead_duration_default(),
            high_freq_threshold_duration: high_freq_threshold_default(),
            high_freq_assign_interval: high_freq_assign_interval_default(),
            publish_max_retries: publish_max_retries_default(),
            publish_retry_interval: publish_retry_interval_default(),
            stream_max_len: stream_max_len_default(),
        }
    }
}

fn executor_key_default() -> String {
    "jobx:executor".to_string()
}

fn scan_interval_default() -> Duration {
    Duration::from_secs(60)
}

fn query_start_earlier_duration_default() -> Duration {
    Duration::from_hours(3 * 24)
}

fn query_end_later_duration_default() -> Duration {
    Duration::from_secs(60)
}

fn max_assign_lag_duration_default() -> Duration {
    Duration::from_mins(60)
}

fn assign_lead_duration_default() -> Duration {
    Duration::from_secs(60)
}

fn high_freq_threshold_default() -> Duration {
    Duration::from_secs(60)
}

fn high_freq_assign_interval_default() -> Duration {
    Duration::from_mins(5)
}

fn publish_max_retries_default() -> u32 {
    5
}

fn publish_retry_interval_default() -> Duration {
    Duration::from_secs(8)
}

fn stream_max_len_default() -> usize {
    10000
}
