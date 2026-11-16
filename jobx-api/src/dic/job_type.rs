use robotech::macros::dic;
use strum::{AsRefStr, Display, EnumString};

/// 被 `JobxJobVo` / `JobxTaskVo` 用作字段，所以必须声明 `default`
/// （`#[vo]` 会生成 `i16 → 枚举` 的 `From<i16>`，越界需要兜底变体）。
#[dic(default = Manual)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, Display, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum JobType {
    /// 手动分派
    Manual = 0,
    /// cron 表达式
    Cron = 1,
    /// 固定延迟
    FixedDelay = 2,
    /// 固定频率
    FixedRate = 3,
}

impl JobType {
    /// 高频任务是否按「一次分派 = 一段连续执行」的方式运行
    ///
    /// 只有固定间隔类计划（`FixedDelay` / `FixedRate`）会把一次分派批成一段连续执行；
    /// 间隔小于高频阈值时，服务端按 `high_freq_assign_interval` 推进、执行器在段内循环。
    ///
    /// `Cron` 永远**按拍点每拍执行一次**：cron 的语义就是「每个拍点跑一次」，
    /// 即便拍点间隔小于高频阈值（例如每 30 秒），也只是执行得更频繁，
    /// 不能把 5 分钟内的 10 个拍点合并成一段循环。所以 `Cron` 返回 `false`。
    ///
    /// 注意：`jobx_job.high_freq` 字段仍然保留「是否高频」的展示含义（列表标记、筛选），
    /// 是否批成一段要同时看类型，不能只看这个标记。
    pub fn batches_high_freq_segment(self) -> bool {
        matches!(self, JobType::FixedDelay | JobType::FixedRate)
    }
}
