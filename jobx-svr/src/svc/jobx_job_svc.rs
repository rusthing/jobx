use crate::config::{get_jobx_scheduler_config, JobxSchedulerConfig};
use chrono::{TimeZone, Utc};
use cron::Schedule;
use jobx_api::dic::JobType;
use jobx_api::dto::JobxJobModifyDto;
use robotech::api;
use robotech::macros::svc;
use robotech::db::get_db_conn;
use robotech::redis::publish_to_stream_if_not_exists;
use sea_orm::ActiveValue;
use std::str::FromStr;
use tracing::{info, warn};
use wheel_rs::time_utils::now_ms;

/// # 任务计划服务
///
/// 负责任务计划的增删改查、调度计算以及定时扫描发布到 Redis Stream。
#[svc(before_add = Self::before_add, before_modify = Self::before_modify)]
pub struct JobxJobSvc;

impl JobxJobSvc {
    /// # 计算下一次分派时间（核心共享逻辑）
    ///
    /// 根据任务类型，从起始时间戳开始计算下一次分派时间戳（即提前于执行时间的分派时间）。
    /// Manual 返回 `None`。超过有效结束时间也返回 `None`。
    ///
    /// 计算逻辑：
    /// - Cron：根据 cron 表达式计算下一次执行时间，再减去分派提前量
    /// - FixedDelay / FixedRate：以 `after_ms` 作为下一次执行时间（不在此处加 `interval_duration`；
    ///   `add` 流程保证新增任务立即分派，间隔推进由 `recalc_next_assign_ms` 在调用前完成）
    ///
    /// ## 参数
    ///
    /// * `job_type` - 任务类型（Cron / FixedDelay / FixedRate / Manual）。
    /// * `cron` - Cron 表达式，Cron 类型时必填。
    /// * `interval_duration` - 固定间隔时长，FixedDelay / FixedRate 时必填。
    /// * `valid_begin_ms` - 有效开始时间戳，None 表示从 0 开始。
    /// * `valid_end_ms` - 有效结束时间戳，None 表示无上限。
    /// * `assign_lead_ms` - 分派提前毫秒数，用于提前分派任务。
    /// * `after_ms` - 起始时间戳（毫秒），用于计算下一次分派时间。
    ///   - 新增时：传入当前时间 `now_ms`
    ///   - 重新计算时：传入上一次的 `next_assign_ms`
    ///
    /// ## 返回值
    ///
    /// * `Ok((Some(u64), bool))` — 下一次分派时间戳（毫秒）、是否高频任务。
    /// * `Ok((None, false))` — Manual 类型，或无下一次（已超出有效期）。
    /// * `Err` — 参数校验失败。
    fn calc_next_assign(
        job_type: JobType,
        cron: Option<String>,
        interval_duration: Option<api::Duration>,
        valid_begin_ms: Option<U64>,
        valid_end_ms: Option<U64>,
        assign_lead_ms: Option<U64>,
        mut after_ms: u64,
    ) -> Result<(Option<u64>, bool), SvcError> {
        let JobxSchedulerConfig {
            assign_lead_duration: assign_lead_duration_default,
            high_freq_threshold_duration,
            ..
        } = *get_jobx_scheduler_config()?;

        // 如果无有效开始或结束时间，也就是没有约束边界
        let valid_begin_ms: u64 = valid_begin_ms.map(|e| e.into()).unwrap_or(0);
        let valid_end_ms: u64 = valid_end_ms.map(|e| e.into()).unwrap_or(u64::MAX);
        let assign_lead_ms: u64 = assign_lead_ms
            .map(|e| e.into())
            .unwrap_or(assign_lead_duration_default.as_millis() as u64);

        if valid_begin_ms > after_ms {
            after_ms = valid_begin_ms;
        }

        let (next_exec_ms, is_high_freq_task) = match job_type {
            JobType::Cron => {
                let cron = cron.ok_or_else(|| {
                    validator::ValidationError::new("Cron类型的任务计划必须提供cron表达式")
                })?;

                let schedule = Schedule::from_str(&cron)
                    .map_err(|_| validator::ValidationError::new("cron表达式解析失败"))?;

                let after_datetime = Utc
                    .timestamp_millis_opt(after_ms as i64)
                    .single()
                    .ok_or_else(|| validator::ValidationError::new("时间戳无效"))?;

                let mut iterator = schedule.after(&after_datetime);

                let next_datetime = iterator.next();
                let next_datetime = if let Some(next_datetime) = next_datetime {
                    next_datetime
                } else {
                    return Ok((None, false));
                };

                let next_exec_ms = next_datetime.timestamp_millis() as u64;

                let is_high_freq_task = iterator
                    .next()
                    .map(|next_next_datetime| {
                        next_next_datetime.timestamp_millis() as u64 - next_exec_ms
                            < high_freq_threshold_duration.as_millis() as u64
                    })
                    .unwrap_or(false);

                (next_exec_ms, is_high_freq_task)
            }

            JobType::FixedDelay | JobType::FixedRate => {
                let interval_duration = interval_duration.ok_or_else(|| {
                    validator::ValidationError::new("固定间隔任务必须提供间隔时间")
                })?;

                // add 流程中 after_ms = max(valid_begin_ms, now_ms)，此时不应加 interval，
                // 以保证新增任务立即被分派。interval 的推进由 recalc_next_assign_ms 在调用前完成。
                let next_exec_ms = after_ms;
                let is_high_freq_task =
                    interval_duration.as_duration() < high_freq_threshold_duration;

                (next_exec_ms, is_high_freq_task)
            }
            _ => {
                // 前面已经排除了Manual类型，不会运行到这里
                unreachable!()
            }
        };

        if next_exec_ms > valid_end_ms {
            Err(validator::ValidationError::new("无下一次执行时间"))?
        }

        let next_assign_ms = next_exec_ms - assign_lead_ms;

        Ok((Some(next_assign_ms), is_high_freq_task))
    }

    /// # 计算任务调度信息
    ///
    /// 根据任务类型、cron 表达式或固定间隔、有效时间范围，计算下一次分派时间戳及是否为高频任务。
    /// Manual 类型不参与调度。
    ///
    /// ## 参数（使用双层 Option 语义）
    ///
    /// 各参数外层 `Option` 表示"该字段是否存在于 modify DTO 中"（来自 `Option<Option<T>>` 的 flatten），
    /// 内层 `Option` 表示字段的实际值（`None` 即设为 null）。此设计用于区分"DTO 中未传此字段"
    /// 和"DTO 中显式设置为 null"两种语义：
    /// - `Some(Some(v))`：DTO 中设置为具体值 `v`
    /// - `Some(None)`：DTO 中显式设置为 null
    /// - `None`：DTO 中未包含此字段，需从数据库原值合并
    ///
    /// * `job_type` - 任务类型（Manual / Cron / FixedDelay / FixedRate）。
    /// * `cron` - Cron 表达式，Cron 类型必填。
    /// * `interval_duration` - 固定间隔时长，FixedDelay / FixedRate 类型必填。
    /// * `valid_begin_ms` - 有效开始时间戳，None 表示从 0 开始。
    /// * `valid_end_ms` - 有效结束时间戳，None 表示无上限。
    /// * `assign_lead_ms` - 分派提前毫秒数，None 时使用配置默认值。
    ///
    /// ## 返回值
    ///
    /// * `(Option<u64>, bool)` — 下次分派时间戳、是否高频任务。
    fn calc_job_schedule(
        job_type: JobType,
        cron: Option<Option<String>>,
        interval_duration: Option<Option<api::Duration>>,
        valid_begin_ms: Option<Option<U64>>,
        valid_end_ms: Option<Option<U64>>,
        assign_lead_ms: Option<Option<U64>>,
    ) -> Result<(Option<u64>, bool), SvcError> {
        // 先把外层的Option flatten出来
        let cron = cron.flatten();
        let interval_duration = interval_duration.flatten();
        let valid_begin_ms = valid_begin_ms.flatten();
        let valid_end_ms = valid_end_ms.flatten();
        let assign_lead_ms = assign_lead_ms.flatten();

        let now_ms: u64 = now_ms();

        let (next_assign_ms, is_high_freq_task) = Self::calc_next_assign(
            job_type,
            cron,
            interval_duration,
            valid_begin_ms,
            valid_end_ms,
            assign_lead_ms,
            now_ms,
        )?;
        if next_assign_ms.is_none() {
            Err(validator::ValidationError::new("无下一次执行时间"))?
        }

        Ok((next_assign_ms, is_high_freq_task))
    }

    /// # add 前置钩子：计算新增任务计划的调度字段
    ///
    /// 根据任务类型计算并设置 `assign_lead_ms`、`next_assign_ms`、`high_freq` 等调度派生字段。
    /// Manual 类型任务不参与调度，对应字段置为 None。
    /// 此钩子由 `#[svc]` 宏在 `add` 方法中自动调用，调用时机在校验 DTO 之前。
    ///
    /// ## 参数
    ///
    /// * `add_dto` - 任务计划新增 DTO，钩子会修改其中的调度派生字段并通过返回值写回。
    ///
    /// ## 返回值
    ///
    /// * `Ok(JobxJobAddDto)` — 已完成调度字段计算的 DTO，将交还给 `#[svc]` 宏继续执行后续流程。
    /// * `Err` — 参数不完整或调度计算失败。
    async fn before_add(mut add_dto: JobxJobAddDto) -> Result<JobxJobAddDto, SvcError> {
        let job_type = add_dto
            .job_type
            .ok_or_else(|| validator::ValidationError::new("job_type不能为空"))?;
        if job_type == JobType::Manual {
            add_dto.assign_lead_ms = Some(None);
            add_dto.high_freq = Some(None);
            add_dto.next_assign_ms = Some(None);
        } else {
            let JobxSchedulerConfig {
                assign_lead_duration: assign_lead_duration_default,
                ..
            } = *get_jobx_scheduler_config()?;

            let cron = add_dto.cron.clone();
            let interval_duration = add_dto.interval_duration.clone();
            let valid_begin_ms = add_dto.valid_begin_ms.clone();
            let valid_end_ms = add_dto.valid_end_ms.clone();

            // assign_lead_ms 在 DTO 中声明为 u32（宏映射成 U32），而调度计算统一使用 U64，这里先归一化再回写
            let assign_lead_ms = add_dto.assign_lead_ms.flatten().unwrap_or(api::U32::from(
                assign_lead_duration_default.as_millis() as u32,
            ));

            let (next_assign_ms, high_freq) = Self::calc_job_schedule(
                job_type,
                cron,
                interval_duration,
                valid_begin_ms,
                valid_end_ms,
                Some(Some(U64::from(assign_lead_ms.value() as u64))),
            )?;
            add_dto.assign_lead_ms = Some(Some(assign_lead_ms));
            add_dto.next_assign_ms =
                Some(next_assign_ms.map(|next_assign_ms| next_assign_ms.into()));
            add_dto.high_freq = Some(Some(high_freq));
        }

        Ok(add_dto)
    }

    /// # modify 前置钩子：计算修改任务计划时的调度字段
    ///
    /// 根据 DTO 中设置的调度参数，与数据库原值合并后重新计算 `next_assign_ms`、`high_freq`。
    /// Manual 类型任务不参与调度，对应字段置为 None。
    /// 此钩子由 `#[svc]` 宏在 `modify` 方法中自动调用，调用时机在校验 DTO 之前。
    ///
    /// 合并策略：DTO 中未传的字段（外层 `Option` 为 `None`）使用数据库原值，
    /// 显式设置为 null 的字段（`Some(None)`）或传了具体值的字段（`Some(Some(v))`）以 DTO 为准。
    ///
    /// ## 参数
    ///
    /// * `modify_dto` - 任务计划修改 DTO，钩子会修改其中的调度派生字段并通过返回值写回。
    ///
    /// ## 返回值
    ///
    /// * `Ok(JobxJobModifyDto)` — 已完成调度字段计算的 DTO，将交还给 `#[svc]` 宏继续执行后续流程。
    /// * `Err` — 参数不完整、原记录不存在或调度计算失败。
    async fn before_modify(
        mut modify_dto: JobxJobModifyDto,
    ) -> Result<JobxJobModifyDto, SvcError> {
        let modify_dto_clone = modify_dto.clone();
        let id = modify_dto_clone
            .id
            .ok_or_else(|| validator::ValidationError::new("修改操作必须提供id"))?;

        // next_assign_ms 是派生字段（基于调度参数和 now 计算），每次 modify 都需刷新
        if let Some(job_type) = modify_dto_clone.job_type.clone()
            && job_type == JobType::Manual
        {
            modify_dto.assign_lead_ms = Some(None);
            modify_dto.high_freq = Some(None);
            modify_dto.next_assign_ms = Some(None);
        } else {
            let db = get_db_conn()?;

            let existing = JobxJobDao::get_by_id::<_, JobxJobVo>(id, db.as_ref())
                .await?
                .ok_or_else(|| SvcError::NotFound(id.to_string()))?;

            let job_type = modify_dto_clone.job_type.unwrap_or(existing.job_type);
            if job_type == JobType::Manual {
                modify_dto.assign_lead_ms = Some(None);
                modify_dto.high_freq = Some(None);
                modify_dto.next_assign_ms = Some(None);
            } else {
                let cron = modify_dto_clone.cron.or(Some(existing.cron.clone()));
                let interval_duration = modify_dto_clone
                    .interval_duration
                    .or(Some(existing.interval_duration.clone()));
                let valid_begin_ms = modify_dto_clone
                    .valid_begin_ms
                    .or(Some(existing.valid_begin_ms.clone()));
                let valid_end_ms = modify_dto_clone
                    .valid_end_ms
                    .or(Some(existing.valid_end_ms.clone()));
                let assign_lead_ms = modify_dto_clone
                    .assign_lead_ms
                    .map(|v| v.map(|v| U64::from(v.value() as u64)))
                    .or(Some(
                        existing
                            .assign_lead_ms
                            .map(|v| U64::from(v.value() as u64)),
                    ));

                let (next_assign_ms, high_freq) = Self::calc_job_schedule(
                    job_type,
                    cron,
                    interval_duration,
                    valid_begin_ms,
                    valid_end_ms,
                    assign_lead_ms,
                )?;

                modify_dto.next_assign_ms =
                    Some(next_assign_ms.map(|next_assign_ms| next_assign_ms.into()));
                modify_dto.high_freq = Some(Some(high_freq));
            }
        }

        Ok(modify_dto)
    }

    /// # 刷新任务计划的下次分派时间
    ///
    /// Worker 在成功领取任务后调用，以当前 `next_assign_ms` 为起点向后推一个调度周期。
    /// 仅更新 `next_assign_ms` 和审计字段，不触碰 `high_freq` 等其他参数。
    ///
    /// ## 推进策略
    ///
    /// 在调用 `calc_next_assign` 之前先根据任务类型对起始时间 (`after_ms`) 做预偏移：
    /// - **高频任务**：`next_assign_ms + high_freq_assign_interval`（避免高频任务过于密集地重新分派）
    /// - **普通任务**：`next_assign_ms + assign_lead_ms + interval_duration`
    ///   - `assign_lead_ms` 与 `calc_next_assign` 末尾的减法抵消，使计算基准对齐执行时间点
    ///   - `interval_duration` 使 FixedDelay/FixedRate 任务真正向前推进（Cron 为 0，不影响）
    ///
    /// ## 参数
    ///
    /// * `job_id` - 任务计划 ID。
    /// * `db` - 数据库连接（事务）。
    #[db_unwrap(transaction_required)]
    #[log_call]
    pub async fn recalc_next_assign_ms<C>(
        job_id: U64,
        #[skip_log] db: Option<&C>,
    ) -> Result<(), SvcError>
    where
        C: ConnectionTrait,
    {
        let existing = JobxJobDao::get_by_id::<_, JobxJobVo>(job_id, db)
            .await?
            .ok_or_else(|| SvcError::NotFound(job_id.to_string()))?;

        let next_assign_ms = if existing.job_type == JobType::Manual {
            None
        } else {
            let after_ms = if let Some(next_assign_ms) = existing.next_assign_ms {
                let JobxSchedulerConfig {
                    high_freq_assign_interval,
                    assign_lead_duration: assign_lead_duration_default,
                    ..
                } = *get_jobx_scheduler_config()?;
                if existing.high_freq.unwrap_or(false) {
                    next_assign_ms.value() + high_freq_assign_interval.as_millis() as u64
                } else {
                    let lead_ms = existing
                        .assign_lead_ms
                        .map(|v| v.value() as u64)
                        .unwrap_or(assign_lead_duration_default.as_millis() as u64);
                    let interval_duration_ms = existing
                        .interval_duration
                        .as_ref()
                        .map(|d| d.as_duration())
                        .unwrap_or_default()
                        .as_millis() as u64;
                    next_assign_ms.value() + lead_ms + interval_duration_ms
                }
            } else {
                return Ok(());
            };

            let (next_assign_ms, _high_freq) = Self::calc_next_assign(
                existing.job_type,
                existing.cron.clone(),
                existing.interval_duration,
                existing.valid_begin_ms,
                existing.valid_end_ms,
                existing.assign_lead_ms.map(|v| U64::from(v.value() as u64)),
                after_ms,
            )?;
            next_assign_ms.map(|next_assign_ms| next_assign_ms as i64)
        };

        let active_model = ActiveModel {
            id: ActiveValue::Set(job_id.into()),
            next_assign_ms: ActiveValue::Set(next_assign_ms),
            updator_id: ActiveValue::Set(existing.updator_id.value() as i64),
            ..Default::default()
        };

        JobxJobDao::update(active_model, db).await?;
        Ok(())
    }

    /// # 扫描可发布任务并推送到 Redis Stream
    ///
    /// 查询当前时间窗口内 `next_assign_ms` 到期的任务，序列化为 JSON 后
    /// 以 `{stream_key}:{executor_code}` 为 key 调用 `publish_to_stream_if_not_exists` 幂等发布。
    /// 每次发布在独立的 tokio 任务中执行，失败自动重试（次数与间隔由配置控制）。
    ///
    /// ## 参数
    ///
    /// * `config` - 调度配置（含 stream_key、时间窗口、重试次数与间隔等参数）。
    /// * `db` - 数据库连接。
    ///
    /// ## 返回值
    ///
    /// 无返回值。查询结果为空时直接返回 `Ok(())`。
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