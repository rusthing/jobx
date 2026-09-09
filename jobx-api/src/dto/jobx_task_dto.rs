use robotech::macros::crud_dto;

#[crud_dto]
pub struct JobxTaskDto {
    /// 任务计划ID
    pub job_id: Option<i64>,
    /// 任务状态: 0=运行中, 1=成功, 2=失败
    pub status: i16,
    /// 预定执行时间戳
    pub scheduled_ts: i64,
    /// 开始执行时间戳
    pub start_ts: Option<i64>,
    /// 结束执行时间戳
    pub end_ts: Option<i64>,
}