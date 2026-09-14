use sea_orm::{ColIdx, DbErr, QueryResult, TryGetError, TryGetable};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumString};
use utoipa::ToSchema;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    AsRefStr,
    Display,
    EnumString,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[strum(serialize_all = "kebab-case")]
pub enum TaskStatus {
    /// 运行中
    Running = 0,
    /// 成功
    Success = 1,
    /// 失败
    Failed = 2,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Running
    }
}

impl TaskStatus {
    pub const fn value(self) -> i16 {
        self as i16
    }

    pub fn from_i16(value: i16) -> Option<Self> {
        match value {
            0 => Some(TaskStatus::Running),
            1 => Some(TaskStatus::Success),
            2 => Some(TaskStatus::Failed),
            _ => None,
        }
    }
}

impl From<TaskStatus> for i16 {
    fn from(v: TaskStatus) -> Self {
        v as i16
    }
}

impl From<i16> for TaskStatus {
    fn from(v: i16) -> Self {
        Self::from_i16(v).unwrap_or_default()
    }
}

impl TryGetable for TaskStatus {
    fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
        let value = <i16 as TryGetable>::try_get_by(res, idx)?;
        Self::from_i16(value)
            .ok_or_else(|| TryGetError::DbErr(DbErr::Custom(format!("invalid task status value: {value}"))))
    }
}