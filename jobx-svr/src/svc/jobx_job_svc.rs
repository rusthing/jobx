use crate::config::{get_jobx_scheduler_config, JobxSchedulerConfig};
use chrono::{TimeZone, Utc};
use cron::Schedule;
use jobx_api::dic::JobType;
use jobx_api::dto::JobxJobModifyDto;
use robotech::api;
use robotech::macros::svc;
use robotech::redis::publish_to_stream_if_not_exists;
use std::str::FromStr;
use std::time::Duration;
use tracing::{info, warn};
use wheel_rs::time_utils::now_ms;

/// # 任务计划服务
/// 负责任务计划的增删改查、调度计算以及定时扫描发布到 Redis Stream。
#[svc(skip [add, modify])]
pub struct JobxJobSvc;

impl JobxJobSvc {
    /// # 计算任务调度信息
    /// 根据任务类型、cron 表达式或固定间隔、有效时间范围，计算下一次分派时间戳及是否为高频任务。
    /// Manual 类型不参与调度，返回 None。
    /// - job_type: 任务类型（Manual/Cron/FixedDelay/FixedRate）
    /// - cron: cron 表达式，Cron 类型必填
    /// - interval_duration: 固定间隔时长，FixedDelay/FixedRate 类型必填
    /// - valid_begin_ms: 有效开始时间戳，None 表示从 0 开始
    /// - valid_end_ms: 有效结束时间戳，None 表示无上限
    /// - assign_lead_ms: 分派提前毫秒数，None 时使用 assign_lead_duration_default
    /// - assign_lead_duration_default: 默认分派提前时长
    /// - high_freq_threshold_duration: 高频阈值，任务间隔低于此值视为高频任务
    /// 返回 (是否高频任务, 下次分派时间戳)，Manual 类型返回 (None, None)
    fn calc_job_schedule(
        job_type: JobType,
        cron: Option<Option<String>>,
        interval_duration: Option<Option<api::Duration>>,
        valid_begin_ms: Option<Option<U64>>,
        valid_end_ms: Option<Option<U64>>,
        assign_lead_ms: Option<Option<U64>>,
        assign_lead_duration_default: Duration,
        high_freq_threshold_duration: Duration,
    ) -> Result<(Option<bool>, Option<U64>), SvcError> {
        let now_ms: U64 = now_ms().into();
        let high_freq_threshold_ms = high_freq_threshold_duration.as_millis();

        let cron = cron.flatten();
        let interval_duration = interval_duration.flatten();
        let valid_begin_ms = valid_begin_ms.flatten();
        let valid_end_ms = valid_end_ms.flatten();
        let assign_lead_ms = assign_lead_ms
            .flatten()
            .map(u64::from)
            .unwrap_or(assign_lead_duration_default.as_millis() as u64);

        let has_range = valid_begin_ms.is_some() || valid_end_ms.is_some();
        let mut valid_begin_ms = valid_begin_ms.unwrap_or(U64(0));
        let valid_end_ms = valid_end_ms.unwrap_or(U64::max());

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
                    if now_ms > valid_begin_ms && now_ms < valid_end_ms {
                        valid_begin_ms = now_ms;
                    }
                    let begin_dt = Utc
                        .timestamp_millis_opt(valid_begin_ms.into())
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

                let first_exec_ms: U64 = first.timestamp_millis().into();

                // 检查第一次执行是否超出有效结束时间
                if first_exec_ms > valid_end_ms {
                    Err(validator::ValidationError::new(
                        "cron表达式在有效时间范围内无下一次执行时间",
                    ))?;
                }

                // 检查是否为高频率任务
                let high_freq = match upcoming.next() {
                    Some(second) if U64(second.timestamp_millis() as u64) <= valid_end_ms => {
                        let interval: u64 = (second.timestamp_millis() - first_exec_ms).into();
                        interval < high_freq_threshold_ms as u64
                    }
                    _ => false,
                };

                // 计算下一次分配时间戳
                let next_assign_ms: u64 = (first_exec_ms - assign_lead_ms).into();

                Ok((Some(high_freq), Some(next_assign_ms.into())))
            }

            JobType::FixedDelay | JobType::FixedRate => {
                let interval_duration = interval_duration.ok_or_else(|| {
                    validator::ValidationError::new("固定延迟或固定频率的任务计划必须提供间隔时间")
                })?;

                // 计算第一次执行时间
                let first_exec_ms = if has_range {
                    // 如果当前时间已超出有效结束时间，则设为 0
                    if now_ms > valid_end_ms {
                        Err(validator::ValidationError::new(
                            "在有效时间范围内无下一次执行时间",
                        ))?;
                    }
                    // 如果开始时间大于当前时间，则第一次执行时间为开始时间
                    // 否则，第一次执行时间为当前时间
                    if now_ms < valid_begin_ms {
                        valid_begin_ms
                    } else {
                        now_ms
                    }
                } else {
                    now_ms
                };

                // 检查是否为高频率任务
                let interval_ms = interval_duration.as_millis();
                let high_freq = interval_ms < high_freq_threshold_ms;

                // 计算下一次分配时间戳
                let next_assign_ms: u64 = (first_exec_ms - assign_lead_ms).into();

                Ok((Some(high_freq), Some(next_assign_ms.into())))
            }
        }
    }

    /// # 从旧分派时间往后推，计算下一次分派时间
    /// 与 calc_job_schedule 的区别：
    /// - calc_job_schedule 基于 now 算第一次分派，还会算 high_freq 标志
    /// - 此函数基于旧的 next_assign_ms 往后推一个调度周期，只返回下一次分派时间
    /// - 不触碰 high_freq，只关心"从 from_ms 开始后还有没有可分派的时间点"
    /// - from_ms: 旧的 next_assign_ms（已经减过 lead 的）
    fn calc_next_ms_after(
        job_type: JobType,
        cron: Option<String>,
        interval_duration: Option<api::Duration>,
        valid_begin_ms: Option<U64>,
        valid_end_ms: Option<U64>,
        assign_lead_ms: Option<U64>,
        assign_lead_duration_default: Duration,
        from_ms: U64,
    ) -> Result<Option<U64>, SvcError> {
        let assign_lead_ms = assign_lead_ms
            .map(u64::from)
            .unwrap_or(assign_lead_duration_default.as_millis() as u64);

        let valid_begin_ms = valid_begin_ms.unwrap_or(U64(0));
        let valid_end_ms = valid_end_ms.unwrap_or(U64::max());

        match job_type {
            JobType::Manual => Ok(None),

            JobType::Cron => {
                let cron = cron.ok_or_else(|| {
                    validator::ValidationError::new("Cron类型的任务计划必须提供cron表达式")
                })?;

                let schedule = Schedule::from_str(&cron)
                    .map_err(|_| validator::ValidationError::new("cron表达式解析失败"))?;

                let from_exec_ms = u64::from(from_ms) + assign_lead_ms;
                let begin_dt = Utc
                    .timestamp_millis_opt(from_exec_ms as i64)
                    .single()
                    .ok_or_else(|| validator::ValidationError::new("时间戳无效"))?;

                let next_exec = schedule.after(&begin_dt).next().ok_or_else(|| {
                    validator::ValidationError::new("cron表达式无下一次执行时间")
                })?;

                let next_exec_ms: U64 = next_exec.timestamp_millis().into();

                if next_exec_ms > valid_end_ms {
                    return Ok(None);
                }

                let _ = valid_begin_ms;
                let next_assign_ms: U64 = (next_exec_ms.value() - assign_lead_ms).into();
                Ok(Some(next_assign_ms))
            }

            JobType::FixedDelay | JobType::FixedRate => {
                let interval_duration = interval_duration.ok_or_else(|| {
                    validator::ValidationError::new("固定间隔任务必须提供间隔时间")
                })?;

                let interval_ms = interval_duration.as_millis() as u64;
                let from_exec_ms = u64::from(from_ms) + assign_lead_ms;
                let next_exec_ms = from_exec_ms + interval_ms;

                if next_exec_ms > u64::from(valid_end_ms) {
                    return Ok(None);
                }

                let _ = valid_begin_ms;
                let next_assign_ms: U64 = (next_exec_ms - assign_lead_ms).into();
                Ok(Some(next_assign_ms))
            }
        }
    }

    /// # 添加任务计划
    /// 校验输入参数、计算调度信息后，将任务计划写入数据库。
    /// - add_dto: 任务计划新增 DTO
    /// - db: 数据库连接（事务）
    /// 返回新创建的任务计划视图对象。
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

        let JobxSchedulerConfig {
            high_freq_threshold_duration,
            assign_lead_duration: assign_lead_duration_default,
            ..
        } = *get_jobx_scheduler_config()?;

        let job_type = add_dto
            .job_type
            .ok_or_else(|| validator::ValidationError::new("job_type不能为空"))?;
        let cron = add_dto.cron.clone();
        let interval_duration = add_dto.interval_duration.clone();
        let valid_begin_ms = add_dto.valid_begin_ms.clone();
        let valid_end_ms = add_dto.valid_end_ms.clone();
        let assign_lead_ms = add_dto.assign_lead_ms.clone();

        let (high_freq, next_assign_ms) = Self::calc_job_schedule(
            job_type,
            cron,
            interval_duration,
            valid_begin_ms,
            valid_end_ms,
            assign_lead_ms,
            assign_lead_duration_default,
            high_freq_threshold_duration,
        )?;
        add_dto.high_freq = Some(high_freq);
        add_dto.next_assign_ms = Some(next_assign_ms);

        let active_model: ActiveModel = add_dto.into();
        let one = JobxJobVo::from(JobxJobDao::insert(active_model, db).await?);
        Ok(Ro::success("添加成功".to_string()).extra(Some(one)))
    }

    /// # 修改任务计划
    /// 校验输入参数，若调度相关字段（job_type/cron/间隔/有效时间/提前时长）有变更，
    /// 则与数据库原值合并后重新计算调度信息，最后更新数据库。
    /// - modify_dto: 任务计划修改 DTO
    /// - db: 数据库连接（事务）
    /// 返回更新后的任务计划视图对象。
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

        let JobxSchedulerConfig {
            high_freq_threshold_duration,
            assign_lead_duration: assign_lead_duration_default,
            ..
        } = *get_jobx_scheduler_config()?;

        // next_assign_ms 是派生字段（基于调度参数和 now 计算），每次 modify 都需刷新
        // 先从 DTO 取出已设置的调度字段值（在 into() 消费 DTO 之前）
        let job_type = modify_dto.job_type;
        let cron = modify_dto.cron.clone();
        let interval_duration = modify_dto.interval_duration.clone();
        let valid_begin_ms = modify_dto.valid_begin_ms.clone();
        let valid_end_ms = modify_dto.valid_end_ms.clone();
        let assign_lead_ms = modify_dto.assign_lead_ms.clone();

        // 获取原记录以获取可能未在modify_dto中设置的字段
        let existing = JobxJobDao::get_by_id::<_, JobxJobVo>(id, db)
            .await?
            .ok_or_else(|| SvcError::NotFound(id.to_string()))?;

        // 合并：优先使用DTO中的新值，否则使用数据库原值
        let job_type = job_type.unwrap_or(existing.job_type);
        let cron = cron.or(Some(existing.cron.clone()));
        let interval_duration = interval_duration.or(Some(existing.interval_duration.clone()));
        let valid_begin_ms = valid_begin_ms.or(Some(existing.valid_begin_ms.clone()));
        let valid_end_ms = valid_end_ms.or(Some(existing.valid_end_ms.clone()));
        let assign_lead_ms =
            assign_lead_ms.or(Some(existing.assign_lead_ms.clone()));

        let (high_freq, next_assign_ms) = Self::calc_job_schedule(
            job_type,
            cron,
            interval_duration,
            valid_begin_ms,
            valid_end_ms,
            assign_lead_ms,
            assign_lead_duration_default,
            high_freq_threshold_duration,
        )?;
        modify_dto.high_freq = Some(high_freq);
        modify_dto.next_assign_ms = Some(next_assign_ms);

        let active_model: ActiveModel = modify_dto.into();
        let one = JobxJobVo::from(JobxJobDao::update(active_model, db).await?);
        Ok(Ro::success("修改成功".to_string()).extra(Some(one)))
    }

    /// # 刷新任务计划的下次分派时间
    /// 基于 job 现有调度参数，以当前 next_assign_ms 为起点，往后推一个调度周期。
    /// 只更新 next_assign_ms 和审计字段，不触碰 high_freq 等其他参数。
    /// 用于 worker 成功接收任务后推进下次分派时间，与 modify 是两条独立路径。
    #[db_unwrap(transaction_required)]
    #[log_call]
    pub async fn refresh_next_assign_ms<C>(
        job_id: U64,
        #[skip_log] db: Option<&C>,
    ) -> Result<(), SvcError>
    where
        C: ConnectionTrait,
    {
        let existing = JobxJobDao::get_by_id::<_, JobxJobVo>(job_id, db)
            .await?
            .ok_or_else(|| SvcError::NotFound(job_id.to_string()))?;

        let from_ms = existing
            .next_assign_ms
            .ok_or_else(|| SvcError::Runtime(anyhow::anyhow!("job.next_assign_ms 为空")))?;

        let JobxSchedulerConfig {
            assign_lead_duration: assign_lead_duration_default,
            ..
        } = *get_jobx_scheduler_config()?;

        let next_assign_ms = Self::calc_next_ms_after(
            existing.job_type,
            existing.cron.clone(),
            existing.interval_duration,
            existing.valid_begin_ms,
            existing.valid_end_ms,
            existing.assign_lead_ms,
            assign_lead_duration_default,
            from_ms,
        )?
        .ok_or_else(|| {
            SvcError::Runtime(anyhow::anyhow!("job 已过有效期，无法计算下次分派时间"))
        })?;

        use jobx_api::mo::jobx_job::ActiveModel;
        use sea_orm::ActiveValue;

        let active_model = ActiveModel {
            id: ActiveValue::Set(job_id.into()),
            next_assign_ms: ActiveValue::Set(Some(next_assign_ms.value() as i64)),
            updator_id: ActiveValue::Set(existing.updator_id.value() as i64),
            update_ms: ActiveValue::Set(now_ms() as i64),
            ..Default::default()
        };

        JobxJobDao::update(active_model, db).await?;
        Ok(())
    }

    /// # 扫描可发布任务并推送到 Redis Stream
    /// 查询当前时间窗口内 `next_assign_ms` 到期的任务，序列化为 JSON 后
    /// 以 `{stream_key}:{executor_code}` 为 key 调用 `publish_to_stream_if_not_exists` 幂等发布。
    /// 每次发布在独立的 tokio 任务中执行，失败自动重试（次数与间隔由配置控制）。
    /// - config: 调度配置（含 stream_key、时间窗口、重试次数与间隔等参数）
    /// - db: 数据库连接
    /// 无返回值，查询结果为空时直接返回 Ok(())。
    #[db_unwrap]
    #[log_call]
    pub async fn scan_and_publish<C>(
        config: &JobxSchedulerConfig,
        #[skip_log] db: Option<&C>,
    ) -> Result<(), SvcError>
    where
        C: ConnectionTrait,
    {
        let now = now_ms() as i64;
        let begin = now - config.query_start_earlier_duration.as_millis() as i64;
        let end = now + config.query_end_later_duration.as_millis() as i64;

        let jobs = JobxJobDao::list_publishable(begin, end, now, db).await?;

        if jobs.is_empty() {
            return Ok(());
        }

        for job in &jobs {
            let job = job.clone();
            let stream_key = config.executor_key.clone();
            let publish_max_retries = config.publish_max_retries;
            let publish_retry_interval = config.publish_retry_interval;
            tokio::spawn(async move {
                // let task_add_dto = JobxTaskAddDto::builder()
                //     .task_type(TaskType::Scheduled)
                //     .job_id(Some(job.id.into()))
                //     .executor_code(job.executor_code.clone())
                //     .executor_instance(None)
                //     .exec_params(job.params.clone())
                //     .job_type(job.job_type)
                //     .high_freq(job.high_freq)
                //     .cron(job.cron.clone())
                //     .interval_duration(job.interval_duration)
                //     .valid_begin_ms(job.valid_begin_ms.map(|v| v.into()))
                //     .valid_end_ms(job.valid_end_ms.map(|v| v.into()))
                //     .assign_lead_ms(job.assign_lead_ms)
                //     .scheduled_exec_start_ms(
                //         job.next_assign_ms
                //             .zip(job.assign_lead_ms)
                //             .map(|(next_ms, lead_ms)| {
                //                 U64::from(u64::from(next_ms) + lead_ms)
                //             }),
                //     )
                //     .assign_ms(now_ms().into())
                //     ._current_user_id(job.updator_id.into())
                //     .build();
                //
                // // 任务记录创建失败不 return，可能是重复发布（心跳），继续走发布流程
                // match JobxTaskSvc::add::<DatabaseTransaction>(task_add_dto, None).await {
                //     Ok(_) => {
                //         info!("添加任务到数据库成功: executor_code={}", job.executor_code);
                //     }
                //     Err(e) => {
                //         warn!(
                //             "添加任务到数据库失败(可能是重复发布): executor_code={}, error={:?}",
                //             job.executor_code, e
                //         );
                //     }
                // }

                let key = format!("{}:{}", stream_key, job.executor_code);
                let payload = match serde_json::to_string(&job) {
                    Ok(p) => p,
                    Err(e) => {
                        warn!(
                            "序列化任务失败: executor_code={}, error={:?}",
                            job.executor_code, e
                        );
                        return;
                    }
                };

                let mut published = false;
                for attempt in 0..publish_max_retries {
                    match publish_to_stream_if_not_exists(&key, &[("payload", &payload)]).await {
                        Ok(Some(msg_id)) => {
                            info!(
                                "发布任务到 Redis Stream: job_name={}, msg_id={}, executor_code={}",
                                job.name, msg_id, job.executor_code
                            );
                            published = true;
                            break;
                        }
                        Ok(None) => {
                            warn!(
                                "任务已存在: job_name={}, executor_code={}",
                                job.name, job.executor_code
                            );
                            published = true;
                            break;
                        }
                        Err(e) => {
                            warn!(
                                "发布任务到 Redis Stream 失败(第{}次): job_name={}, executor_code={}, error={:?}",
                                attempt + 1,
                                job.name,
                                job.executor_code,
                                e
                            );
                            if attempt < publish_max_retries - 1 {
                                tokio::time::sleep(publish_retry_interval).await;
                            }
                        }
                    }
                }

                // // 高频任务：发布成功后才推进下次分派时间（心跳间隔）
                // // 如果 next_assign_ms 更新失败，下轮扫描仍然会查到旧值并重新发布，
                // // 虽然频率比预期高，但保证了容错性
                // if published && job.high_freq == Some(true) {
                //     let new_next = job
                //         .next_assign_ms
                //         .map(|n| {
                //             (u64::from(n) as i64)
                //                 .wrapping_add(high_freq_assign_interval.as_millis() as i64)
                //         })
                //         .unwrap_or_else(|| {
                //             now_ms() as i64 + high_freq_assign_interval.as_millis() as i64
                //         });
                //
                //     let modify_dto = JobxJobModifyDto::builder()
                //         .id(job.id)
                //         .next_assign_ms(Some(U64(new_next as u64)))
                //         ._current_user_id(job.updator_id)
                //         .build();
                //     let active_model: ActiveModel = modify_dto.into();
                //
                //     // 在控制台层面的错误，由于任务已经发布，仅做告警
                //     const UPDATE_MAX_RETRIES: u32 = 3;
                //     for attempt in 0..UPDATE_MAX_RETRIES {
                //         match robotech::db::get_db_conn() {
                //             Ok(db_conn) => {
                //                 match JobxJobDao::update(active_model.clone(), db_conn.as_ref())
                //                     .await
                //                 {
                //                     Ok(_) => {
                //                         info!(
                //                             "更新高频任务下次分派时间成功: executor_code={}, next_assign_ms={}",
                //                             job.executor_code, new_next
                //                         );
                //                         break;
                //                     }
                //                     Err(e) => {
                //                         warn!(
                //                             "更新高频任务下次分派时间失败(第{}次): executor_code={}, error={:?}",
                //                             attempt + 1,
                //                             job.executor_code,
                //                             e
                //                         );
                //                         if attempt < UPDATE_MAX_RETRIES - 1 {
                //                             tokio::time::sleep(Duration::from_secs(1)).await;
                //                         }
                //                     }
                //                 }
                //             }
                //             Err(e) => {
                //                 warn!(
                //                     "获取数据库连接失败(第{}次): executor_code={}, error={:?}",
                //                     attempt + 1,
                //                     job.executor_code,
                //                     e
                //                 );
                //                 if attempt < UPDATE_MAX_RETRIES - 1 {
                //                     tokio::time::sleep(Duration::from_secs(1)).await;
                //                 }
                //             }
                //         }
                //     }
                // }

                if !published {
                    warn!(
                        "发布任务到 Redis Stream 最终失败(已重试{}次): job_name={}, executor_code={}",
                        publish_max_retries, job.name, job.executor_code
                    );
                }
            });
        }

        Ok(())
    }
}