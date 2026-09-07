use robotech::macros::crud_dto;

#[crud_dto]
pub struct ScheduleDto {
    /// 关联任务ID
    pub job_id: u64,
    /// Cron 表达式
    pub cron: String,
    /// 是否启用
    pub enabled: bool,
    /// 备注
    pub remark: Option<String>,
}
