use robotech::macros::dic;
use strum::{AsRefStr, Display, EnumString};

/// 被 `JobxTaskVo` 用作字段，所以必须声明 `default`。
#[dic(default = Immediate)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, Display, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum TaskType {
    /// 立即执行
    Immediate = 1,
    /// 计划执行
    Scheduled = 2,
}
