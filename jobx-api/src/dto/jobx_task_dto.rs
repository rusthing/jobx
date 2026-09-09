use robotech::macros::crud_dto;

#[crud_dto]
pub struct JobxTaskDto {
    /// 任务计划ID
    pub job_id: Option<i64>,
    /// 任务状态: 0=待分派, 1=待处理, 2=运行中, 3=成功, 4=失败, 5=超时
    pub status: Option<i64>,
    /// 开始执行时间戳
    pub start_ts: Option<i64>,
    /// 结束执行时间戳
    pub end_ts: Option<i64>,
}
