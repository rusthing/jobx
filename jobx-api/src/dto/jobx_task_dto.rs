use crate::dic::{JobType, TaskStatus, TaskType};
use robotech::api::Duration;
use robotech::macros::crud_dto;

#[crud_dto]
pub struct JobxTaskDto {
    /// 任务类型: 1=立即执行, 2=计划执行
    pub task_type: TaskType,
    /// 任务计划ID
    pub job_id: Option<u64>,
    /// 任务状态: 0=运行中, 1=成功, 2=失败
    #[db_default]
    pub status: TaskStatus,
    /// 分派时间戳
    pub assign_ms: u64,
    /// 预定开始执行时间戳
    pub scheduled_exec_start_ms: Option<u64>,
    /// 执行器编码
    pub executor_code: String,
    /// 执行器实例
    pub executor_instance: Option<String>,
    /// 执行参数
    pub exec_params: Option<String>,
    /// 计划类型: 0=手动分派, 1=cron表达式, 2=固定延迟, 3=固定频率
    pub job_type: JobType,
    /// 是否高频任务
    pub high_freq: Option<bool>,
    /// cron表达式
    pub cron: Option<String>,
    /// 固定间隔时间
    pub interval_duration: Option<Duration>,
    /// 有效开始时间戳
    pub valid_begin_ms: Option<u64>,
    /// 有效结束时间戳
    pub valid_end_ms: Option<u64>,
    /// 分派提前毫秒数
    pub assign_lead_ms: Option<u64>,
    /// 执行详情
    pub exec_detail: Option<String>,
    /// 开始执行时间戳
    pub exec_start_ms: Option<u64>,
    /// 结束执行时间戳
    pub exec_end_ms: Option<u64>,
}