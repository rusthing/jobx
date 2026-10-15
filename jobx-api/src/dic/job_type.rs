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
