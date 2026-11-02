use robotech::macros::crud_dto;

/// # 任务分组
///
/// 分组只描述「这批计划归谁管」，与执行器编码（谁跑）、调度策略（多久跑）正交。
/// 计划的分组归属在 `JobxJobDto::group_id` 上，为空即未分组。
#[crud_dto]
pub struct JobxGroupDto {
    /// 名称
    /// @len 1..=50
    pub name: String,
    /// 备注
    /// @len 0..=50
    pub remark: Option<String>,
}
