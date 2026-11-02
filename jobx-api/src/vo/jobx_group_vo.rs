use robotech::macros::vo;

/// # 任务分组
///
/// 计划通过 `jobx_job.group_id` 挂在分组下；计划侧只在 [`super::JobxJobVo`] 里
/// 存分组 ID，分组名走接口查询或前端按 ID 映射，不做冗余，避免改名后两边不一致。
#[vo]
pub struct JobxGroupVo {
    /// ID
    pub id: u64,
    /// 名称
    pub name: String,
    /// 备注
    pub remark: Option<String>,
    /// 创建者ID
    pub creater_id: u64,
    /// 创建时间戳
    pub create_ms: u64,
    /// 更新者ID
    pub updater_id: u64,
    /// 更新时间戳
    pub update_ms: u64,
}
