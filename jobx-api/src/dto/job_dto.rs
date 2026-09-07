use robotech::macros::crud_dto;

#[crud_dto]
pub struct JobDto {
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
}
