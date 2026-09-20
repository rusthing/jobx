use robotech::macros::svc;
use robotech::db::get_db_conn;
use crate::svc::JobxJobSvc;

#[svc]
pub struct JobxTaskSvc;

impl JobxTaskSvc {
    /// # Worker 领取任务：创建 task + 推进 job 下次分派时间（同一事务）
    /// - add_dto: 任务新增 DTO（含 job_id）
    /// 返回新创建的任务视图对象。
    #[log_call]
    pub async fn take(
        add_dto: JobxTaskAddDto,
    ) -> Result<Ro<JobxTaskVo>, SvcError> {
        add_dto.validate()?;

        let db_conn = get_db_conn()?;
        let txn = begin_transaction(db_conn.as_ref()).await?;

        let active_model: ActiveModel = add_dto.into();
        let task_model = JobxTaskDao::insert(active_model, &txn).await?;
        let task_one = JobxTaskVo::from(task_model);

        let job_id = task_one
            .job_id
            .ok_or_else(|| anyhow::anyhow!("task.job_id 为空"))?;
        JobxJobSvc::refresh_next_assign_ms::<sea_orm::DatabaseTransaction>(
            job_id,
            Some(&txn),
        )
        .await?;

        txn.commit().await.map_err(|e| SvcError::Runtime(e.into()))?;

        Ok(Ro::success("领取成功".to_string()).extra(Some(task_one)))
    }
}