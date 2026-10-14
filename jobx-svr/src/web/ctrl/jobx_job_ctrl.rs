use robotech::macros::ctrl;

#[ctrl]
struct JobxJobCtrl;

use crate::vo::JobxTaskVo;

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