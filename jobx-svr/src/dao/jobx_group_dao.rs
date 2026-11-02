use robotech::macros::dao;

/// # 任务分组
///
/// ## 外键
/// - 被 `jobx_job.group_id` 引用：组里还有计划时删除/更新受限
///
/// ## 唯一约束
/// - `name`：分组名全局唯一。分组是给人看的归类维度，允许重名就等于允许
///   「运维组」和「运维」并存，列表里会分出两个点开内容几乎一样的组。
///
/// 约束名由框架按 `ak_{列名}_{表名}` 拼出（PostgreSQL 报错时直接回查这个名字），
/// 所以 Key 的 `a:Code` 在模型里也必须写成列名 `name`。
#[dao(
    unique_keys: [
        ("name", "名称"),
    ],
    like_columns: [
        Column::Name,
        Column::Remark,
    ],
)]
pub struct JobxGroupDao;
