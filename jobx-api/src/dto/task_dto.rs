use robotech::macros::crud_dto;

#[crud_dto]
pub struct TaskDto {
    /// 关联任务ID
    pub job_id: u64,
    /// 任务状态
    pub status: String,
    /// 执行结果
    pub result: Option<String>,
    /// 备注
    pub remark: Option<String>,
}
