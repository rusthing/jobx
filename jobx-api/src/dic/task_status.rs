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
    /// 跳过
    ///
    /// 一次分派被领走、但**一次都没执行**：高频任务开始执行时已经越过
    /// `exec_deadline_ms`（调度积压/执行器排队），或等循环首轮前就收到停机信号。
    /// 它既不是成功也不是失败，单独一种状态，避免把「没跑」算进成功率。
    Skipped = 3,
}
