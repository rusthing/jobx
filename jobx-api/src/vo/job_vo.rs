use robotech::macros::vo;

#[vo]
pub struct JobVo {
    /// ID
    pub id: u64,
    /// 任务名称
    pub name: String,
    /// 任务分组
    pub group: String,
    /// 任务类型
    pub job_type: String,
    /// 任务参数(JSON)
    pub params: Option<String>,
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
