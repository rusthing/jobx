use robotech::macros::vo;

#[vo]
pub struct TaskVo {
    /// ID
    pub id: u64,
    /// 关联任务ID
    pub job_id: u64,
    /// 任务状态
    pub status: String,
    /// 执行结果
    pub result: Option<String>,
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
