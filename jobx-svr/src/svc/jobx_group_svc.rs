use robotech::macros::svc;

/// # 任务分组服务
///
/// 纯 CRUD：分组不参与调度计算，也没有派生字段需要在钩子里维护。
///
/// 「组里还有计划时不能删」由数据库外键 `on delete restrict` 兜底 —— 那条约束是
/// 唯一的事实来源，服务层不再写一遍存在性检查（写了也只是在约束前面多一次查询，
/// 并发下仍然要靠约束）。
#[svc]
pub struct JobxGroupSvc;
