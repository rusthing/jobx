use crate::dto::JobxTaskAddDto;
use robotech::api::U64;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Worker 领取任务的请求体
///
/// 在任务新增 DTO 之外额外带上「发布这条消息时计划的版本」（`jobx_job.update_ms`），
/// 服务端据此判断消息是否已经过期。
///
/// ## 为什么需要版本
///
/// 调度的推进是**异步**的：服务端把计划快照推给 Redis Stream，执行器可能隔一会儿才消费。
/// 这段时间里计划完全可能被改期、暂停或删除，此时按旧快照建出来的任务会：
/// 落在旧的预定执行时刻、多推一拍节奏（`recalc_next_assign_ms` 按当前计划再往前推一格）。
/// 带上版本后，服务端发现「消息里的版本 ≠ 当前计划的版本」就直接丢弃这条消息。
///
/// 版本用 `update_ms` 而不是单独的版本列：计划每次被修改（含 `recalc_next_assign_ms`
/// 推进 `next_assign_ms`）都会刷 `update_ms`，天然单调，不需要额外的模型字段和迁移。
///
/// `job_update_ms` 为空表示执行器是老版本、未携带版本，此时跳过校验（兼容滚动升级）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxTaskTakeDto {
    /// 任务新增 DTO（服务端据此创建 `jobx_task` 行）
    pub task: JobxTaskAddDto,
    /// 发布消息时计划的 `update_ms`
    pub job_update_ms: Option<U64>,
}
