use robotech::macros::crud_dto;

#[crud_dto]
pub struct JobxTaskDto {
    /// 任务类型: 1=立即执行, 2=计划执行
    pub task_type: i16,
    /// 任务计划ID
    pub job_id: Option<u64>,
    /// 任务状态: 0=运行中, 1=成功, 2=失败
    pub status: i16,
    /// 分派时间戳
    pub assign_ts: u64,
    /// 预定分派时间戳
    pub scheduled_assign_ts: Option<u64>,
    /// 执行详情
    pub exec_detail: Option<String>,
    /// 开始执行时间戳
    pub exec_start_ts: Option<u64>,
    /// 结束执行时间戳
    pub exec_end_ts: Option<u64>,
}