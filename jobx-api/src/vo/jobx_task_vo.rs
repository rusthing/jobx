use robotech::macros::vo;

#[vo]
pub struct JobxTaskVo {
    /// ID
    pub id: u64,
    /// 任务计划ID
    pub job_id: Option<u64>,
    /// 任务状态: 0=待分派, 1=待处理, 2=运行中, 3=成功, 4=失败, 5=超时
    pub status: Option<i64>,
    /// 开始执行时间戳
    pub start_ts: Option<i64>,
    /// 结束执行时间戳
    pub end_ts: Option<i64>,
    /// 创建者ID
    pub creator_id: u64,
    /// 创建时间
    pub create_ts: u64,
    /// 更新者ID
    pub updator_id: u64,
    /// 更新时间
    pub update_ts: u64,
}
