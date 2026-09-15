use crate::config::SchedulerConfig;
use crate::svc::JobxTaskSvc;
use jobx_api::dic::TaskType;
use jobx_api::dto::JobxTaskAddDto;
use robotech::macros::svc;
use robotech::redis::publish_to_stream;
use sea_orm::DatabaseTransaction;
use tracing::{info, warn};
use wheel_rs::time_utils::now_ts;

#[svc]
pub struct JobxJobSvc;
impl JobxJobSvc {
    #[db_unwrap]
    #[log_call]
    pub async fn scan_and_publish<C>(
        config: &SchedulerConfig,
        #[skip_log] db: Option<&C>,
    ) -> Result<(), SvcError>
    where
        C: ConnectionTrait,
    {
        let now = now_ts()? as i64;
        let begin = now - config.query_start_earlier_duration.as_millis() as i64;
        let end = now + config.query_end_later_duration.as_millis() as i64;

        let jobs = JobxJobDao::find_publishable(begin, end, now, db).await?;

        if jobs.is_empty() {
            return Ok(());
        }

        for job in &jobs {
            let job = job.clone();
            let stream_key = config.stream_key.clone();
            tokio::spawn(async move {
                let now = match now_ts() {
                    Ok(ts) => ts,
                    Err(e) => {
                        warn!("获取当前时间戳失败: job_code={}, error={:?}", job.code, e);
                        return;
                    }
                };
                // 添加任务到数据库
                let task_add_dto = JobxTaskAddDto::builder()
                    .task_type(TaskType::Scheduled)
                    .job_id(Some(job.id.into()))
                    .scheduled_assign_ts(Some(job.next_assign_ts.into()))
                    .assign_ts(now.into())
                    ._current_user_id(job.updator_id.into())
                    .build();

                match JobxTaskSvc::add::<DatabaseTransaction>(task_add_dto, None).await {
                    Ok(_) => {
                        info!("添加任务到数据库成功: job_code={}", job.code);
                    }
                    Err(e) => {
                        warn!("添加任务到数据库失败: job_code={}, error={:?}", job.code, e);
                        return;
                    }
                }

                // 发布任务到 Redis Stream
                let payload = match serde_json::to_string(&job) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("序列化任务失败: job_code={}, error={:?}", job.code, e);
                        return;
                    }
                };
                match publish_to_stream(&stream_key, &[("payload", &payload)]).await {
                    Ok(msg_id) => {
                        info!(
                            "发布任务到 Redis Stream: job_code={}, job_name={}, msg_id={}",
                            job.code, job.name, msg_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "发布任务到 Redis Stream 失败: job_code={}, error={:?}",
                            job.code, e
                        );
                    }
                }
            });
        }

        Ok(())
    }
}