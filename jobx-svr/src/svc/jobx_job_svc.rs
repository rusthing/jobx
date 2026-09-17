use crate::config::{get_jobx_scheduler_config, SchedulerConfig};
use crate::svc::JobxTaskSvc;
use chrono::{TimeZone, Utc};
use cron::Schedule;
use jobx_api::dic::{JobType, TaskType};
use jobx_api::dto::JobxTaskAddDto;
use robotech::api;
use robotech::macros::svc;
use robotech::redis::publish_to_stream;
use sea_orm::DatabaseTransaction;
use std::str::FromStr;
use std::time::Duration;
use tracing::{info, warn};
use wheel_rs::time_utils::now_ms;

#[svc(skip [add, modify])]
pub struct JobxJobSvc;

impl JobxJobSvc {
    fn calc_job_schedule(
        job_type: JobType,
        cron: Option<Option<String>>,
        interval_duration: Option<Option<api::Duration>>,
        valid_begin_ts: Option<Option<U64>>,
        valid_end_ts: Option<Option<U64>>,
        assign_lead_duration: Option<Option<api::Duration>>,
        assign_lead_duration_default: Duration,
        high_freq_threshold_duration: Duration,
    ) -> Result<(Option<bool>, Option<U64>), SvcError> {
        let now_ms: U64 = now_ms().into();
        let high_freq_threshold_ms = high_freq_threshold_duration.as_millis();

        let cron = cron.flatten();
        let interval_duration = interval_duration.flatten();
        let valid_begin_ts = valid_begin_ts.flatten();
        let valid_end_ts = valid_end_ts.flatten();
        let assign_lead_duration = assign_lead_duration
            .flatten()
            .map(|d| d.into())
            .unwrap_or(assign_lead_duration_default);

        let has_range = valid_begin_ts.is_some() || valid_end_ts.is_some();
        let mut begin_ts = valid_begin_ts.unwrap_or(U64(0));
        let end_ts = valid_end_ts.unwrap_or(U64::max());

        match job_type {
            JobType::Manual => Ok((None, None)),

            JobType::Cron => {
                let cron = cron.ok_or_else(|| {
                    validator::ValidationError::new("Cron类型的任务计划必须提供cron表达式")
                })?;

                let schedule = Schedule::from_str(&cron)
                    .map_err(|_| validator::ValidationError::new("cron表达式解析失败"))?;

                // 当前时间在有效开始时间内，从当前时间开始查找，否则从有效开始时间起查找
                let mut upcoming = if has_range {
                    if now_ms > begin_ts && now_ms < end_ts {
                        begin_ts = now_ms;
                    }
                    let begin_dt = Utc
                        .timestamp_millis_opt(begin_ts.into())
                        .single()
                        .ok_or_else(|| validator::ValidationError::new("有效开始时间戳无效"))?;
                    schedule.after(&begin_dt)
                } else {
                    schedule.upcoming(Utc)
                };

                let first = upcoming.next().ok_or_else(|| {
                    validator::ValidationError::new(if has_range {
                        "cron表达式在有效时间范围内无下一次执行时间"
                    } else {
                        "cron表达式无下一次执行时间"
                    })
                })?;

                let first_exec_ts: U64 = first.timestamp_millis().into();

                // 检查第一次执行是否超出有效结束时间
                if first_exec_ts > end_ts {
                    Err(validator::ValidationError::new(
                        "cron表达式在有效时间范围内无下一次执行时间",
                    ))?;
                }

                // 检查是否为高频率任务
                let high_freq = match upcoming.next() {
                    Some(second) if U64(second.timestamp_millis() as u64) <= end_ts => {
                        let interval: u64 = (second.timestamp_millis() - first_exec_ts).into();
                        interval < high_freq_threshold_ms as u64
                    }
                    _ => false,
                };

                // 计算下一次分配时间戳
                let next_assign_ts: u64 = (first_exec_ts - assign_lead_duration.as_millis()).into();

                Ok((Some(high_freq), Some(next_assign_ts.into())))
            }

            JobType::FixedDelay | JobType::FixedRate => {
                let interval_duration = interval_duration.ok_or_else(|| {
                    validator::ValidationError::new("固定延迟或固定频率的任务计划必须提供间隔时间")
                })?;

                // 计算第一次执行时间
                let first_exec_ts = if has_range {
                    // 如果当前时间已超出有效结束时间，则设为 0
                    if now_ms > end_ts {
                        Err(validator::ValidationError::new(
                            "在有效时间范围内无下一次执行时间",
                        ))?;
                    }
                    // 如果开始时间大于当前时间，则第一次执行时间为开始时间
                    // 否则，第一次执行时间为当前时间
                    if now_ms < begin_ts { begin_ts } else { now_ms }
                } else {
                    now_ms
                };

                // 检查是否为高频率任务
                let interval_ms = interval_duration.as_millis();
                let high_freq = interval_ms < high_freq_threshold_ms;

                // 计算下一次分配时间戳
                let next_assign_ts: u64 = (first_exec_ts - assign_lead_duration.as_millis()).into();

                Ok((Some(high_freq), Some(next_assign_ts.into())))
            }
        }
    }

    #[db_unwrap(transaction_required)]
    #[log_call]
    pub async fn add<C>(
        mut add_dto: JobxJobAddDto,
        #[skip_log] db: Option<&C>,
    ) -> Result<Ro<JobxJobVo>, SvcError>
    where
        C: ConnectionTrait,
    {
        add_dto.validate()?;

        let SchedulerConfig {
            high_freq_threshold_duration,
            assign_lead_duration: assign_lead_duration_default,
            ..
        } = *get_jobx_scheduler_config()?;

        let job_type = add_dto
            .job_type
            .ok_or_else(|| validator::ValidationError::new("job_type不能为空"))?;
        let cron = add_dto.cron.clone();
        let interval_duration = add_dto.interval_duration.clone();
        let valid_begin_ts = add_dto.valid_begin_ts.clone();
        let valid_end_ts = add_dto.valid_end_ts.clone();
        let assign_lead_duration = add_dto.pre_assign_duration.clone();

        let (high_freq, next_assign_ts) = Self::calc_job_schedule(
            job_type,
            cron,
            interval_duration,
            valid_begin_ts,
            valid_end_ts,
            assign_lead_duration,
            assign_lead_duration_default,
            high_freq_threshold_duration,
        )?;
        add_dto.high_freq = Some(high_freq);
        add_dto.next_assign_ts = Some(next_assign_ts);

        let active_model: ActiveModel = add_dto.into();
        let one = JobxJobVo::from(JobxJobDao::insert(active_model, db).await?);
        Ok(Ro::success("添加成功".to_string()).extra(Some(one)))
    }

    #[db_unwrap(transaction_required)]
    #[log_call]
    pub async fn modify<C>(
        mut modify_dto: JobxJobModifyDto,
        #[skip_log] db: Option<&C>,
    ) -> Result<Ro<JobxJobVo>, SvcError>
    where
        C: ConnectionTrait,
    {
        modify_dto.validate()?;

        let id = modify_dto
            .id
            .ok_or_else(|| validator::ValidationError::new("修改操作必须提供id"))?;

        let SchedulerConfig {
            high_freq_threshold_duration,
            assign_lead_duration: assign_lead_duration_default,
            ..
        } = *get_jobx_scheduler_config()?;

        // 只有任务计划相关字段有变更时才重新计算
        let has_schedule_change = modify_dto.job_type.is_some()
            || modify_dto.cron.is_some()
            || modify_dto.interval_duration.is_some()
            || modify_dto.valid_begin_ts.is_some()
            || modify_dto.valid_end_ts.is_some()
            || modify_dto.pre_assign_duration.is_some();
        if has_schedule_change {
            // 先从 DTO 取出已设置的调度字段值（在 into() 消费 DTO 之前）
            // 注意：字符串字段需 clone 为自有值，避免借用冲突
            let job_type = modify_dto.job_type;
            let cron = modify_dto.cron.clone();
            let interval_duration = modify_dto.interval_duration.clone();
            let valid_begin_ts = modify_dto.valid_begin_ts.clone();
            let valid_end_ts = modify_dto.valid_end_ts.clone();
            let assign_lead_duration = modify_dto.pre_assign_duration.clone();

            // 获取原记录以获取可能未在modify_dto中设置的字段
            let existing = JobxJobDao::get_by_id::<_, JobxJobVo>(id, db)
                .await?
                .ok_or_else(|| SvcError::NotFound(id.to_string()))?;

            // 合并：优先使用DTO中的新值，否则使用数据库原值
            let job_type = job_type.unwrap_or(existing.job_type);
            let cron = cron.or(Some(existing.cron.clone()));
            let interval_duration = interval_duration.or(Some(existing.interval_duration.clone()));
            let valid_begin_ts = valid_begin_ts.or(Some(existing.valid_begin_ts.clone()));
            let valid_end_ts = valid_end_ts.or(Some(existing.valid_end_ts.clone()));
            let assign_lead_duration =
                assign_lead_duration.or(Some(existing.pre_assign_duration.clone()));

            let (high_freq, next_assign_ts) = Self::calc_job_schedule(
                job_type,
                cron,
                interval_duration,
                valid_begin_ts,
                valid_end_ts,
                assign_lead_duration,
                assign_lead_duration_default,
                high_freq_threshold_duration,
            )?;
            modify_dto.high_freq = Some(high_freq);
            modify_dto.next_assign_ts = Some(next_assign_ts);
        }

        let active_model: ActiveModel = modify_dto.into();
        let one = JobxJobVo::from(JobxJobDao::update(active_model, db).await?);
        Ok(Ro::success("修改成功".to_string()).extra(Some(one)))
    }

    #[db_unwrap]
    #[log_call]
    pub async fn scan_and_publish<C>(
        config: &SchedulerConfig,
        #[skip_log] db: Option<&C>,
    ) -> Result<(), SvcError>
    where
        C: ConnectionTrait,
    {
        let now = now_ms() as i64;
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
                let now = now_ms();

                let task_add_dto = JobxTaskAddDto::builder()
                    .task_type(TaskType::Scheduled)
                    .job_id(Some(job.id.into()))
                    .scheduled_assign_ts(job.next_assign_ts.map(|v| v.into()))
                    .assign_ts(now.into())
                    ._current_user_id(job.updator_id.into())
                    .build();

                match JobxTaskSvc::add::<DatabaseTransaction>(task_add_dto, None).await {
                    Ok(_) => {
                        info!("添加任务到数据库成功: job_code={}", job.executor_code);
                    }
                    Err(e) => {
                        warn!("添加任务到数据库失败: job_code={}, error={:?}", job.executor_code, e);
                        return;
                    }
                }

                let key = format!("{}:{}", stream_key, job.executor_code);
                let payload = match serde_json::to_string(&job) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("序列化任务失败: job_code={}, error={:?}", job.executor_code, e);
                        return;
                    }
                };
                match publish_to_stream(&key, &[("payload", &payload)]).await {
                    Ok(msg_id) => {
                        info!(
                            "发布任务到 Redis Stream: job_code={}, job_name={}, msg_id={}",
                            job.executor_code, job.name, msg_id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "发布任务到 Redis Stream 失败: job_code={}, error={:?}",
                            job.executor_code, e
                        );
                    }
                }
            });
        }

        Ok(())
    }
}