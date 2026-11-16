use robotech::macros::ctrl;

#[ctrl]
struct JobxJobCtrl;

use crate::vo::JobxTaskVo;
use jobx_api::dto::JobxOverviewSummaryDto;

#[utoipa::path(
    get,
    path = "/jobx/overview/summary",
    responses((status = OK, body = Ro<JobxOverviewSummaryDto>))
)]
#[log_call]
pub async fn overview_summary() -> Result<Json<Ro<JobxOverviewSummaryDto>>, CtrlError> {
    // 只读聚合：不传连接，由 `#[db_unwrap]` 自己取全局连接池（不开事务）
    let summary = JobxJobSvc::overview_summary::<DatabaseConnection>(None).await?;
    Ok(Json(Ro::success("查询成功".to_string()).extra(Some(summary))))
}

#[utoipa::path(
    post,
    path = "/jobx/job/trigger",
    request_body = JobxJobTriggerDto,
    responses((status = OK, body = Ro<JobxTaskVo>))
)]
#[log_call]
pub async fn trigger(
    headers: HeaderMap,
    Json(dto): Json<JobxJobTriggerDto>,
) -> Result<Json<Ro<JobxTaskVo>>, CtrlError> {
    let user_id = get_current_user_id(&headers)?;
    let current_ms = get_current_ms(&headers)?;
    let result = JobxJobSvc::trigger(dto.job_id, user_id, current_ms).await?;
    Ok(Json(result))
}