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

impl Default for JobType {
    fn default() -> Self {
        Self::Manual
    }
}

impl JobType {
    pub const fn value(self) -> i16 {
        self as i16
    }

    pub fn from_i16(value: i16) -> Option<Self> {
        match value {
            0 => Some(JobType::Manual),
            1 => Some(JobType::Cron),
            2 => Some(JobType::FixedDelay),
            3 => Some(JobType::FixedRate),
            _ => None,
        }
    }
}

impl From<JobType> for i16 {
    fn from(v: JobType) -> Self {
        v as i16
    }
}

impl TryFrom<i16> for JobType {
    type Error = InvalidJobTypeError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        Self::from_i16(value).ok_or(InvalidJobTypeError(value))
    }
}

impl TryGetable for JobType {
    fn try_get_by<I: ColIdx>(res: &QueryResult, idx: I) -> Result<Self, TryGetError> {
        let value = <i16 as TryGetable>::try_get_by(res, idx)?;
        Self::from_i16(value)
            .ok_or_else(|| TryGetError::DbErr(DbErr::Custom(format!("invalid job type value: {value}"))))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidJobTypeError(pub i16);

impl fmt::Display for InvalidJobTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid job type value: {}", self.0)
    }
}

impl std::error::Error for InvalidJobTypeError {}