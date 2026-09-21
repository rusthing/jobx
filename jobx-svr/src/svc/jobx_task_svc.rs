use crate::svc::JobxJobSvc;
use anyhow::anyhow;
use robotech::macros::svc;

#[svc]
pub struct JobxTaskSvc;

impl JobxTaskSvc {
    /// # Worker 领取任务：创建 task + 推进 job 下次分派时间（同一事务）
    /// - add_dto: 任务新增 DTO（含 job_id）
    /// 返回新创建的任务视图对象。
    #[db_unwrap(transaction_required)]
    pub async fn take<C>(
        add_dto: JobxTaskAddDto,
        db: Option<&C>,
    ) -> Result<Ro<JobxTaskVo>, SvcError>
    where
        C: ConnectionTrait,
    {
        let task_vo: JobxTaskVo =
            if let Some(task_model) = Self::add(add_dto, Some(db)).await?.extra {
                task_model.into()
            } else {
                Err(SvcError::Runtime(anyhow!("创建任务失败")))?
            };
        if let Some(job_id) = task_vo.job_id {
            JobxJobSvc::recalc_next_assign_ms(job_id, Some(db)).await?;
        };
        Ok(Ro::success("领取成功".to_string()).extra(Some(task_vo)))
    }
}
