use crate::svc::JobxJobSvc;
use anyhow::anyhow;
use robotech::macros::svc;

#[svc]
pub struct JobxTaskSvc;

impl JobxTaskSvc {
    /// # Worker 领取任务（在同一事务中创建 task 并推进 job 的下次分派时间）
    ///
    /// 由 Worker 通过 `/jobx/task/take` 接口调用。在一个数据库事务中完成：
    /// 1. 根据 `add_dto` 创建一条任务记录（通过自动生成的 `add` 方法）
    /// 2. 若任务关联了任务计划（`job_id` 不为空），推进该计划的下次分派时间
    ///
    /// 这样做保证任务创建与调度推进的原子性，避免任务已创建但调度未更新的不一致状态。
    ///
    /// ## 参数
    ///
    /// * `add_dto` - 任务新增 DTO，由 Worker 根据收到的任务消息构建（含 `job_id`、`executor_code` 等）
    /// * `db` - 数据库连接（事务）
    ///
    /// ## 返回值
    ///
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