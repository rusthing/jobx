use sea_orm::{ColIdx, DbErr, QueryResult, TryGetError, TryGetable};
use serde::{Deserialize, Serialize};
use std::fmt;
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
pub enum TaskType {
    /// 立即执行
    Immediate = 1,
    /// 计划执行
    Scheduled = 2,
}

impl Default for TaskType {
    fn default() -> Self {
        Self::Immediate
    }
}

impl TaskType {
    pub const fn value(self) -> i16 {
        self as i16
    }

    pub fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(TaskType::Immediate),
            2 => Some(TaskType::Scheduled),
            _ => None,
        }
    }
}

impl From<TaskType> for i16 {
    fn from(v: TaskType) -> Self {
        v as i16
    }
}

impl TryFrom<i16> for TaskType {
    type Error = InvalidTaskTypeError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::from_i16(value).ok_or(InvalidTaskTypeError(value))
    }
}

impl TryGetable for TaskType {
    fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
        let value = <i16 as TryGetable>::try_get_by(res, idx)?;
        Self::from_i16(value)
            .ok_or_else(|| TryGetError::DbErr(DbErr::Custom(format!("invalid task type value: {value}"))))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTaskTypeError(pub i16);

impl fmt::Display for InvalidTaskTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid task type value: {}", self.0)
    }
}

impl std::error::Error for InvalidTaskTypeError {}