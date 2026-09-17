use crate::dic::{TaskStatus, TaskType};
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
    /// 执行详情
    pub exec_detail: Option<String>,
    /// 开始执行时间戳
    pub exec_start_ms: Option<u64>,
    /// 结束执行时间戳
    pub exec_end_ms: Option<u64>,
}