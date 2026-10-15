use robotech::macros::dic;
use strum::{AsRefStr, Display, EnumString};

/// 被 `JobxTaskVo` 用作字段，所以必须声明 `default`。
#[dic(default = Running)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, Display, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum TaskStatus {
    /// 运行中
    Running = 0,
    /// 成功
    Success = 1,
    /// 失败
    Failed = 2,
}
