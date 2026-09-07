use robotech::macros::vo;

#[vo]
pub struct ScheduleVo {
    /// ID
    pub id: u64,
    /// 关联任务ID
    pub job_id: u64,
    /// Cron 表达式
    pub cron: String,
    /// 是否启用
    pub enabled: bool,
    /// 备注
    pub remark: Option<String>,
    /// 创建者ID
    pub creator_id: u64,
    /// 创建时间
    pub create_ts: u64,
    /// 更新者ID
    pub updator_id: u64,
    /// 更新时间
    pub update_ts: u64,
}
