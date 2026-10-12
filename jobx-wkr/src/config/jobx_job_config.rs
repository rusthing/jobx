use derive_setters::Setters;
use jobx_api::dic::JobType;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use typed_builder::TypedBuilder;
use wheel_rs::serde::duration_option_serde;

/// Job 计划配置
///
/// 用于客户端在配置文件中声明要注册的任务计划。
/// 服务启动时会读取所有配置的计划，通过 jobx API 注册到服务端。
#[derive(o2o::o2o, Debug, Clone, Serialize, Deserialize, Setters, TypedBuilder)]
#[owned_into(jobx_api::dto::JobxJobAddDto)]
#[ghosts(
    high_freq: Default::default(),
    valid_begin_ms: Default::default(),
    valid_end_ms: Default::default(),
    report_result: Default::default(),
    next_assign_ms: Default::default(),
    enabled: Default::default(),
    id: Default::default(),
    _current_ms: Default::default(),
    _current_user_id: Default::default(),
)]
#[serde(rename_all = "kebab-case")]
pub struct JobxJobConfig {
    /// 名称
    #[into(Some(~))]
    pub name: String,
    /// 参数
    /// 传递给执行器的业务参数，格式由执行器自行约定（如 JSON）
    #[into(Some(~))]
    #[builder(default)]
    pub params: Option<String>,
    /// 计划类型: 0=手动分派, 1=cron表达式, 2=固定延迟, 3=固定频率
    #[into(Some(~))]
    #[serde(default = "job_type_default")]
    #[builder(default)]
    pub job_type: JobType,
    /// cron 表达式
    /// 当 job_type 为 Cron 时必填
    #[into(Some(~))]
    #[builder(default)]
    pub cron: Option<String>,
    /// 固定间隔时间
    /// 当 job_type 为 FixedDelay 或 FixedRate 时必填
    #[into(Some(~.map(|d| robotech::api::Duration::from(d))))]
    #[serde(with = "duration_option_serde", default = "interval_duration_default")]
    #[builder(default = interval_duration_default())]
    pub interval_duration: Option<Duration>,
    /// 任务执行器的编码
    #[into(Some(~))]
    pub executor_code: String,
    /// 分派提前毫秒数
    /// 调度器会提前此毫秒数将任务分派给执行器，用于预热或预加载
    #[into(Some(~.map(|v| robotech::api::U64::from(v))))]
    #[serde(default = "assign_lead_ms_default")]
    #[builder(default = assign_lead_ms_default())]
    pub assign_lead_ms: Option<u64>,
    /// 备注
    #[into(Some(~))]
    #[serde(default)]
    #[builder(default)]
    pub remark: Option<String>,
}

fn job_type_default() -> JobType {
    JobType::FixedRate
}

fn interval_duration_default() -> Option<Duration> {
    Some(Duration::from_secs(60))
}

fn assign_lead_ms_default() -> Option<u64> {
    Some(60000)
}