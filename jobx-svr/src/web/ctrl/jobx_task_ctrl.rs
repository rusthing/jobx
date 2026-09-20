use robotech::macros::ctrl;

#[ctrl]
struct JobxTaskCtrl;

#[utoipa::path(
    post,
    path = "/jobx/task/take",
    responses((status = OK, body = Ro<JobxTaskVo>))
)]
#[log_call]
pub async fn take(
    headers: HeaderMap,
    Json(mut dto): Json<JobxTaskAddDto>,
) -> Result<Json<Ro<JobxTaskVo>>, CtrlError> {
    dto._current_user_id = get_current_user_id(&headers)?;
    if let Some(ms) = get_current_ms(&headers)? {
        dto._current_ms = Some(ms);
    }
    let result = JobxTaskSvc::take(dto).await?;
    Ok(Json(result))
}