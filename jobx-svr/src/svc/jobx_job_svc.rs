use crate::config::{JobxSchedulerConfig, get_jobx_scheduler_config};
use anyhow::anyhow;
use chrono::{TimeZone, Utc};
use cron::Schedule;
use jobx_api::dic::JobType;
use jobx_api::dto::JobxJobModifyDto;
use jobx_api::mo::jobx_job;
use robotech::api;
use robotech::cst::user_id_cst::SYS_OPERATOR_USER_ID;
use robotech::db::get_db_conn;
use robotech::macros::svc;
use robotech::redis::publish_to_stream_with_maxlen;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use std::str::FromStr;
use tracing::{info, warn};
use wheel_rs::time_utils::now_ms;

use jobx_api::dic::TaskType;
use jobx_api::dto::JobxTaskAddDto;
use jobx_api::vo::JobxTaskVo;
use crate::svc::JobxTaskSvc;

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

        // 用饱和减法而不是裸减：`next_exec_ms < assign_lead_ms` 时裸减会 u64 下溢
        // （debug 构建直接 panic，release 构建回绕成一个巨大的时间戳，比 panic 更难查）。
        // 正常情况下起点是 now / valid_begin_ms / 旧 next_assign_ms，触发不到；
        // 但 `valid_begin_ms` 允许被设成很小的值（甚至 0），一旦有人这么配就炸在这里。
        let next_assign_ms = next_exec_ms.saturating_sub(assign_lead_ms);

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
    async fn before_add<C: ConnectionTrait>(
        mut add_dto: JobxJobAddDto,
        _db: &C,
    ) -> Result<JobxJobAddDto, SvcError> {
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

    /// # 判断本次 modify 是否真的改动了调度参数
    ///
    /// `next_assign_ms` 是派生字段，重算它等于**把调度节奏重置到 `now`**：正在跑的计划
    /// 会立刻多分派一次，已经积累的滞后会被抹掉。所以只在调度参数真的变了才重算——
    /// 改名称、改备注、改执行参数这类操作不该有调度副作用。
    ///
    /// 比较的是「合并后的有效值」与库里的现值。`Option<Option<T>>` 的语义：
    /// 外层 `None` = 本次没传该字段（取库值），`Some(内层)` = 以 DTO 为准
    /// （内层 `None` 表示显式清空）。
    ///
    /// 另外「库里 `next_assign_ms` 为空、但计划本该参与调度」也算需要重算：
    /// 那说明它从没被正确初始化过，不重算就永远不会被分派。
    fn schedule_changed(dto: &JobxJobModifyDto, existing: &JobxJobVo) -> bool {
        if existing.next_assign_ms.is_none() {
            return true;
        }

        let job_type = dto.job_type.unwrap_or(existing.job_type);
        if job_type != existing.job_type {
            return true;
        }

        let cron = match &dto.cron {
            Some(v) => v.clone(),
            None => existing.cron.clone(),
        };
        if cron != existing.cron {
            return true;
        }

        let interval = match &dto.interval_duration {
            Some(v) => v.as_ref().map(|d| d.as_duration()),
            None => existing.interval_duration.as_ref().map(|d| d.as_duration()),
        };
        if interval != existing.interval_duration.as_ref().map(|d| d.as_duration()) {
            return true;
        }

        let valid_begin = match &dto.valid_begin_ms {
            Some(v) => v.as_ref().map(|v| v.value()),
            None => existing.valid_begin_ms.as_ref().map(|v| v.value()),
        };
        if valid_begin != existing.valid_begin_ms.as_ref().map(|v| v.value()) {
            return true;
        }

        let valid_end = match &dto.valid_end_ms {
            Some(v) => v.as_ref().map(|v| v.value()),
            None => existing.valid_end_ms.as_ref().map(|v| v.value()),
        };
        if valid_end != existing.valid_end_ms.as_ref().map(|v| v.value()) {
            return true;
        }

        let assign_lead = match &dto.assign_lead_ms {
            Some(v) => v.as_ref().map(|v| v.value()),
            None => existing.assign_lead_ms.as_ref().map(|v| v.value()),
        };
        assign_lead != existing.assign_lead_ms.as_ref().map(|v| v.value())
    }

    /// # modify 前置钩子：计算修改任务计划时的调度字段
    ///
    /// 根据 DTO 中设置的调度参数，与数据库原值合并后重新计算 `next_assign_ms`、`high_freq`。
    /// Manual 类型任务不参与调度，对应字段置为 None。
    /// 此钩子由 `#[svc]` 宏在 `modify` 方法中自动调用，调用时机在校验 DTO 之前。
    ///
    /// ## 只有调度参数变了才重算
    ///
    /// 见 [`Self::schedule_changed`]。未发生变化时保持 `next_assign_ms` / `high_freq`
    /// 为 `None`（= 不更新该列），调度节奏不受影响。
    ///
    /// 合并策略：DTO 中未传的字段（外层 `Option` 为 `None`）使用数据库原值，
    /// 显式设置为 null 的字段（`Some(None)`）或传了具体值的字段（`Some(Some(v))`）以 DTO 为准。
    ///
    /// ## 参数
    ///
    /// * `modify_dto` - 任务计划修改 DTO，钩子会修改其中的调度派生字段并通过返回值写回。
    /// * `db` - 数据库连接（用框架传入的连接，便于在事务内执行与测试注入）
    ///
    /// ## 返回值
    ///
    /// * `Ok(JobxJobModifyDto)` — 已完成调度字段计算的 DTO，将交还给 `#[svc]` 宏继续执行后续流程。
    /// * `Err` — 参数不完整、原记录不存在或调度计算失败。
    async fn before_modify<C: ConnectionTrait>(
        mut modify_dto: JobxJobModifyDto,
        db: &C,
    ) -> Result<JobxJobModifyDto, SvcError> {
        let id = modify_dto
            .id
            .ok_or_else(|| validator::ValidationError::new("修改操作必须提供id"))?;

        let existing = JobxJobDao::get_by_id::<_, JobxJobVo>(id, db)
            .await?
            .ok_or_else(|| SvcError::NotFound(id.to_string()))?;

        let job_type = modify_dto.job_type.unwrap_or(existing.job_type);
        if job_type == JobType::Manual {
            // 转成（或保持）Manual：调度派生字段必须清空，否则列表里看起来还在排队
            modify_dto.assign_lead_ms = Some(None);
            modify_dto.high_freq = Some(None);
            modify_dto.next_assign_ms = Some(None);
        } else if Self::schedule_changed(&modify_dto, &existing) {
            let cron = modify_dto.cron.clone().or(Some(existing.cron.clone()));
            let interval_duration = modify_dto
                .interval_duration
                .clone()
                .or(Some(existing.interval_duration.clone()));
            let valid_begin_ms = modify_dto
                .valid_begin_ms
                .clone()
                .or(Some(existing.valid_begin_ms.clone()));
            let valid_end_ms = modify_dto
                .valid_end_ms
                .clone()
                .or(Some(existing.valid_end_ms.clone()));
            let assign_lead_ms = modify_dto
                .assign_lead_ms
                .map(|v| v.map(|v| U64::from(v.value() as u64)))
                .or(Some(
                    existing.assign_lead_ms.map(|v| U64::from(v.value() as u64)),
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

        Ok(modify_dto)
    }

    /// # 刷新任务计划的下次分派时间
    ///
    /// Worker 在成功领取任务后调用，以当前 `next_assign_ms` 为起点向后推一个调度周期。
    /// 仅更新 `next_assign_ms` 和审计字段，不触碰 `high_freq` 等其他参数。
    ///
    /// ## 推进策略
    ///
    /// 在调用 `calc_next_assign` 之前先对起始时间 (`after_ms`) 做预偏移，两个分支都形如
    /// `next_assign_ms + assign_lead_ms + <本周期长度>`：
    ///
    /// - **普通任务**：`<本周期长度>` = `interval_duration`（Cron 为 0，由 `schedule.after()`
    ///   自己推进一拍）。
    /// - **高频任务**：`<本周期长度>` = `high_freq_assign_interval`。
    ///
    /// `assign_lead_ms` 必须补上：`calc_next_assign` 末尾会统一减一次
    /// （`next_assign_ms = next_exec_ms - assign_lead_ms`）。不补的话 `next_assign_ms`
    /// 的增量会比本周期长度少一个 lead，执行时刻的间距也跟着少一个 lead。
    ///
    /// 于是两个不变量成立：
    /// - `next_assign_ms` 每次增量 = 本周期长度；
    /// - 相邻 `scheduled_exec_start_ms` 的间距 = 本周期长度，且高频任务的
    ///   `exec_deadline_ms`（= 本段 scheduled + hf_interval）正好等于下一段的 scheduled，
    ///   段与段首尾相接、不重叠。
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

                // 两个分支都要先补回 `assign_lead_ms`：`calc_next_assign` 末尾会把它减掉，
                // 不补就等于让 `next_assign_ms` 的增量少了 lead，执行时刻的间距也跟着少一个 lead。
                let lead_ms = existing
                    .assign_lead_ms
                    .map(|v| v.value() as u64)
                    .unwrap_or(assign_lead_duration_default.as_millis() as u64);

                if existing.high_freq.unwrap_or(false) {
                    // 高频任务：本段执行时段长度就是 `high_freq_assign_interval`，
                    // 所以下一个预定执行时刻 = 上一个 + hf_interval。
                    // 与 `exec_deadline_ms`（= scheduled + hf_interval）严丝合缝：
                    // 上一段结束的时刻正是下一段开始的时刻，不重叠、不留缝。
                    next_assign_ms.value() + lead_ms + high_freq_assign_interval.as_millis() as u64
                } else {
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
            updater_id: ActiveValue::Set(existing.updater_id.value() as i64),
            ..Default::default()
        };

        JobxJobDao::update(active_model, db).await?;
        Ok(())
    }

    /// # 手动触发计划执行
    ///
    /// 根据计划 ID 创建一条立即执行的任务记录。
    #[log_call]
    pub async fn trigger(
        job_id: U64,
        user_id: U64,
        current_ms: Option<U64>,
    ) -> Result<Ro<JobxTaskVo>, SvcError> {
        let db = get_db_conn()?;
        let job_ro = JobxJobSvc::get_by_id(job_id, Some(db.as_ref())).await?;
        let job = job_ro.extra.ok_or_else(|| SvcError::NotFound("计划不存在".to_string()))?;

        let now = current_ms.unwrap_or(U64(now_ms()));

        // 注意：不要设置 `_current_ms`。它只映射 `create_ms`，一旦显式赋值就会绕过 dao
        // `insert` 里「create_ms 未设置时同步填充 create_ms/update_ms」的兜底逻辑，
        // 导致 `update_ms`（not null 且无默认值）落库 NULL 而报错。
        // 审计字段交给 dao 统一填充，与 worker 的 take 路径保持一致。
        let task_dto = JobxTaskAddDto::builder()
            .task_type(TaskType::Immediate)
            .job_id(Some(job_id))
            .assign_ms(now)
            .scheduled_exec_start_ms(Some(now))
            .executor_code(job.executor_code)
            .exec_params(job.params)
            .job_type(job.job_type)
            .high_freq(job.high_freq)
            .cron(job.cron)
            .interval_duration(job.interval_duration)
            .valid_begin_ms(job.valid_begin_ms)
            .valid_end_ms(job.valid_end_ms)
            .assign_lead_ms(job.assign_lead_ms)
            ._current_user_id(user_id)
            .build();

        JobxTaskSvc::add(task_dto, Some(db.as_ref())).await
    }

    /// # 兜底：把「逻辑时间轴已经跑偏」的计划拉回当前时间
    ///
    /// 把所有 `next_assign_ms < now - max_lag_ms` 的启用计划（Manual 除外）的
    /// `next_assign_ms` 直接改成 `now`，返回受影响的行数。
    ///
    /// ## 为什么需要它
    ///
    /// `next_assign_ms` 的推进是**开环**的：起点取旧的 `next_assign_ms` 而不是 `now`
    /// （这样周期不漂移、漏拍可补，见 [`Self::recalc_next_assign_ms`]）。代价是滞后量
    /// 不会自愈，而 `scan_and_publish` 的查询窗口有下界
    /// `now - query_start_earlier_duration`（默认 3 天）：
    ///
    /// - 滞后 < 3 天：还在窗口内，只是每轮扫描只能补一拍，追赶速率被 `scan_interval` 卡住；
    /// - 滞后 > 3 天：`next_assign_ms` 落到窗口外，**永远不会再被选中**——静默停摆，
    ///   日志里一条记录都没有。这是比「执行时刻失真」严重得多的故障。
    ///
    /// 本方法把「静默停摆」变成「拉回当前时间 + 告警」。
    ///
    /// ## 语义：丢弃错过的拍子
    ///
    /// 拉回后 `next_assign_ms = now`，本轮扫描就会命中它，立刻分派一次。
    /// 中间错过的拍子**不补**——补也补不回来（`jobx_task` 里没有对应记录，
    /// 补出来的一串任务执行时刻全在过去，会背靠背地突发执行）。
    ///
    /// Cron 计划会被拉到一个不在 cron 点上的时刻，因此会多跑一次「非整点」的执行；
    /// `take` 里的 `recalc_next_assign_ms` 随后会用 `schedule.after()` 重新对齐到
    /// 真正的 cron 拍点，只影响这一次。
    ///
    /// ## 参数
    ///
    /// * `now` - 当前时间戳（毫秒），由调用方传入以便一次扫描内共用同一基准
    /// * `max_lag_ms` - 允许的最大滞后（毫秒），`JobxSchedulerConfig::max_assign_lag_duration`
    /// * `db` - 数据库连接
    #[log_call]
    pub async fn revive_stale_jobs<C>(
        now: i64,
        max_lag_ms: i64,
        #[skip_log] db: &C,
    ) -> Result<u64, SvcError>
    where
        C: ConnectionTrait,
    {
        let threshold = now - max_lag_ms;
        let rows_affected = jobx_job::Entity::update_many()
            .col_expr(jobx_job::Column::NextAssignMs, Expr::value(now))
            .col_expr(
                jobx_job::Column::UpdaterId,
                Expr::value(SYS_OPERATOR_USER_ID as i64),
            )
            .col_expr(jobx_job::Column::UpdateMs, Expr::value(now))
            .filter(jobx_job::Column::Enabled.eq(true))
            .filter(jobx_job::Column::JobType.ne(JobType::Manual.value()))
            // next_assign_ms 为 NULL 时比较结果是 NULL（不成立），自动被排除
            .filter(jobx_job::Column::NextAssignMs.lt(threshold))
            .exec(db)
            .await
            .map_err(|e| anyhow!("拉回滞后计划失败: {e}"))?
            .rows_affected;

        if rows_affected > 0 {
            warn!(
                "有 {rows_affected} 个计划的 next_assign_ms 滞后超过 {}ms（now={now}），已拉回当前时间",
                max_lag_ms
            );
        }

        Ok(rows_affected)
    }

    /// # 扫描可发布任务并推送到 Redis Stream
    ///
    /// 查询当前时间窗口内 `next_assign_ms` 到期的任务，序列化为 JSON 后
    /// 以 `{stream_key}:{executor_code}` 为 key 发布到 Redis Stream。
    /// 每次发布在独立的 tokio 任务中执行，失败自动重试（次数与间隔由配置控制）。
    ///
    /// ## 幂等性
    ///
    /// 去重依赖 `list_publishable` 里的 `NOT EXISTS(jobx_task...)` 条件：
    /// 只有当「该 job 的当前分派时刻还没有对应的 `jobx_task` 行」时才会被选中
    /// （条件写法有两个坑，见 [`JobxJobDao::list_publishable`] 的文档）。
    /// 但**从发布到 worker 成功 `take` 之间**流里还没有任何 `jobx_task` 行，
    /// 同一 job 会在后续每轮扫描中被重复选中，从而产生重复消息。
    /// 这是**有意接受**的代价，因为重复消息不会造成重复执行：
    /// - 消息 payload 是**发布那一刻的快照**，重复消息携带的 `next_assign_ms` 完全相同，
    ///   因此它们算出的 `scheduled_exec_start_ms` 也完全相同，
    ///   第二条起会被 `jobx_task` 的 `unique (job_id, scheduled_exec_start_ms)` 约束挡掉；
    /// - `take` 与 `recalc_next_assign_ms` 在同一事务内，insert 失败即回滚，
    ///   `next_assign_ms` 不会因为重复消息而多推进。
    ///
    /// 也就是说去重条件的作用只是**少发几条注定会被挡掉的重复消息**，
    /// 真正的正确性兜底是那条唯一约束。
    ///
    /// 若重复消息量很大（worker 长时间掉线后恢复），可以打开执行器的
    /// `last_only`，让 worker 一批只处理最后一条，直接把积压压掉。
    ///
    /// 作为对照，原先用的 `publish_to_stream_if_not_exists`（`XLEN==0` 才发）看似能去重，
    /// 实则把 Stream 变成了「整个生命周期只能装一条消息」的单槽位 ——
    /// `XACK` 不删 Stream 条目，而本仓没有任何 `XDEL`/`XTRIM` 调用，
    /// 于是第一次发布之后 `XLEN` 永远 ≥ 1，后续发布全部被静默拒绝。
    ///
    /// ## 长度控制
    ///
    /// 发布时带 `MAXLEN ~ stream_max_len` 近似裁剪，防止 Stream 无限增长。
    /// `stream_max_len` **优先取计划自身的 `jobx_job.stream_max_len`**，
    /// 该列为空时回落到 `JobxSchedulerConfig::stream_max_len`。
    ///
    /// ## 参数
    ///
    /// * `config` - 调度配置（含 stream_key、时间窗口、重试次数与间隔、Stream 长度上限等参数）。
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

        // 先兜底：把滞后到查询窗口之外（永远发不出去）的计划拉回当前时间。
        // 必须排在 list_publishable 之前，否则被拉回的计划要等下一轮扫描才生效。
        // 失败不中断本轮发布——拉回是兜底手段，不该让正常调度跟着停摆。
        if let Err(e) = Self::revive_stale_jobs(
            now,
            config.max_assign_lag_duration.as_millis() as i64,
            db,
        )
        .await
        {
            warn!("拉回滞后计划失败，本轮仍按原时间窗口扫描: {e:?}");
        }

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
            // 长度上限优先取计划自己的 `stream_max_len`，为空时回落到配置文件的默认值。
            // 让单个计划可以覆盖全局值：突发量特别大的计划调大以免自己的消息被裁掉，
            // 只跑一次的临时计划调小以免长期占位。
            let stream_max_len = job
                .stream_max_len
                .map(|v| v.value() as usize)
                .unwrap_or(config.stream_max_len);
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
                    match publish_to_stream_with_maxlen(
                        &key,
                        &[("payload", &payload)],
                        stream_max_len,
                    )
                    .await
                    {
                        Ok(Some(msg_id)) => {
                            info!(
                                "发布任务到 Redis Stream: key={key}, msg_id={msg_id}, payload={payload}",
                            );
                            published = true;
                            break;
                        }
                        Ok(None) => {
                            // 未使用 NOMKSTREAM，理论上不会走到这里
                            warn!("发布任务到 Redis Stream 未返回消息ID: key={key}");
                            published = true;
                            break;
                        }
                        Err(e) => {
                            warn!(
                                "发布任务到 Redis Stream 失败(第{}次):  key={key}, payload={payload}, error={:?}",
                                attempt + 1,
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

/// # 调度计算与兜底逻辑的测试
///
/// 纯逻辑测试（`calc_next_assign` / `schedule_changed`）不依赖数据库，`cargo test` 直接跑。
///
/// 数据库相关测试需要真实 PostgreSQL，通过环境变量 `JOBX_TEST_DB_URL` 指定连接串，
/// 例如：
///
/// ```text
/// JOBX_TEST_DB_URL=postgres://postgres:postgres@127.0.0.1:5432/jobx_test cargo test -p jobx-svr
/// ```
///
/// 未设置该变量时这些测试打印一行 skip 提示后直接返回，不会失败——避免在没库的环境里
/// 把整个测试套件染红。**它们会在库上 `drop table jobx_job` 重建**，所以务必指向一个
/// 一次性测试库，不要指开发库。
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::set_jobx_scheduler_config;
    use serde_json::json;
    use std::sync::Mutex;

    /// 数据库测试串行执行：它们共用同一张 `jobx_job`，并发跑会互相清表。
    static DB_LOCK: Mutex<()> = Mutex::new(());

    /// 建表语句与 `model/create-table-pgsql.sql` 的 `jobx_job` 保持一致
    /// （VO 会 select 全部列，少一列就查不出来）。
    const CREATE_JOBX_JOB: &str = r#"
create table jobx_job (
   id                   INT8                 not null,
   name                 VARCHAR(50)          not null,
   params               VARCHAR(800)         null,
   job_type             INT2                 not null,
   high_freq            BOOL                 null default false,
   cron                 VARCHAR(30)          null,
   interval_duration    VARCHAR(10)          null,
   valid_begin_ms       INT8                 null,
   valid_end_ms         INT8                 null,
   executor_code        VARCHAR(50)          not null,
   assign_lead_ms       INT4                 null,
   next_assign_ms       INT8                 null,
   stream_max_len       INT4                 null,
   remark               VARCHAR(50)          null,
   enabled              BOOL                 not null default true,
   creater_id           INT8                 not null,
   create_ms            INT8                 not null,
   updater_id           INT8                 not null,
   update_ms            INT8                 not null,
   constraint PK_JOBX_JOB primary key (id),
   constraint AK_NAME_JOBX_JOB unique (name)
)"#;

    fn init_scheduler_config() {
        set_jobx_scheduler_config(JobxSchedulerConfig::default());
    }
    fn lead_default() -> u64 {
        JobxSchedulerConfig::default().assign_lead_duration.as_millis() as u64
    }

    // ================= 纯逻辑：calc_next_assign =================

    /// 固定间隔：`next_assign = after_ms - assign_lead_ms`
    #[test]
    fn fixed_interval_subtracts_lead() {
        init_scheduler_config();
        let (next, high_freq) = JobxJobSvc::calc_next_assign(
            JobType::FixedDelay,
            None,
            Some(api::Duration::from_millis(300_000)),
            None,
            None,
            Some(U64::from(60_000u64)),
            1_000_000,
        )
        .unwrap();

        assert_eq!(next, Some(940_000));
        assert!(!high_freq);
    }

    /// 起点小于提前量时饱和到 0，而不是 u64 下溢
    #[test]
    fn fixed_interval_underflow_saturates() {
        init_scheduler_config();
        let (next, _) = JobxJobSvc::calc_next_assign(
            JobType::FixedRate,
            None,
            Some(api::Duration::from_millis(300_000)),
            None,
            None,
            Some(U64::from(60_000u64)),
            30_000,
        )
        .unwrap();

        assert_eq!(next, Some(0));
    }

    /// 有效开始时间晚于起点时，以有效开始时间为准
    #[test]
    fn valid_begin_clamps_start() {
        init_scheduler_config();
        let (next, _) = JobxJobSvc::calc_next_assign(
            JobType::FixedDelay,
            None,
            Some(api::Duration::from_millis(300_000)),
            Some(U64::from(2_000_000u64)),
            None,
            Some(U64::from(60_000u64)),
            1_000_000,
        )
        .unwrap();

        assert_eq!(next, Some(1_940_000));
    }

    /// 下次执行时刻超过有效结束时间 → 报错（计划已到期）
    #[test]
    fn valid_end_exceeded_errors() {
        init_scheduler_config();
        let result = JobxJobSvc::calc_next_assign(
            JobType::FixedDelay,
            None,
            Some(api::Duration::from_millis(300_000)),
            None,
            Some(U64::from(500_000u64)),
            Some(U64::from(60_000u64)),
            1_000_000,
        );

        assert!(result.is_err(), "超过有效结束时间应当报错");
    }

    /// Cron：落在真正的 cron 拍点上，且 `next_assign + lead` 是 5 分钟的整数倍
    #[test]
    fn cron_picks_next_beat() {
        init_scheduler_config();
        let lead = lead_default();
        let base = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp_millis() as u64;
        let after = base + 1_000;

        let (next, _) = JobxJobSvc::calc_next_assign(
            JobType::Cron,
            Some("0 */5 * * * *".to_string()),
            None,
            None,
            None,
            Some(U64::from(lead)),
            after,
        )
        .unwrap();

        let next = next.expect("cron 应当有下一次");
        let exec_ms = next + lead;
        assert_eq!(
            exec_ms % 300_000,
            0,
            "执行时刻必须落在 5 分钟边界上，实际 {exec_ms}"
        );
        assert_eq!(exec_ms, base + 300_000, "应当是 base 之后的第一个整 5 分钟");
    }

    /// 间隔小于阈值 → 高频任务
    #[test]
    fn high_freq_detected_by_interval() {
        init_scheduler_config();
        let (_, high_freq) = JobxJobSvc::calc_next_assign(
            JobType::FixedDelay,
            None,
            Some(api::Duration::from_millis(30_000)),
            None,
            None,
            Some(U64::from(60_000u64)),
            1_000_000,
        )
        .unwrap();

        assert!(high_freq, "30s 间隔应判定为高频任务");
    }

    // ================= 数据库相关 =================

    async fn connect_test_db() -> Option<sea_orm::DatabaseConnection> {
        let Ok(url) = std::env::var("JOBX_TEST_DB_URL") else {
            eprintln!("跳过数据库测试：未设置 JOBX_TEST_DB_URL");
            return None;
        };
        Some(
            sea_orm::Database::connect(&url)
                .await
                .expect("连接测试库失败，请检查 JOBX_TEST_DB_URL"),
        )
    }

    async fn reset_table(db: &sea_orm::DatabaseConnection) {
        db.execute_unprepared("drop table if exists jobx_task")
            .await
            .expect("drop jobx_task 失败");
        db.execute_unprepared("drop table if exists jobx_job")
            .await
            .expect("drop table 失败");
        db.execute_unprepared(CREATE_JOBX_JOB)
            .await
            .expect("create table 失败");
        // list_publishable 的去重子查询只用到这三列，测试里给最小定义即可
        db.execute_unprepared(
            "create table jobx_task (id INT8 not null primary key, job_id INT8 null, \
             scheduled_exec_start_ms INT8 null)",
        )
        .await
        .expect("create jobx_task 失败");
    }

    /// 插入一条计划。参数按需要覆盖的列给，其余走建表默认值。
    #[allow(clippy::too_many_arguments)]
    async fn insert_job(
        db: &sea_orm::DatabaseConnection,
        id: i64,
        name: &str,
        job_type: i16,
        high_freq: bool,
        cron: Option<&str>,
        interval_duration: Option<&str>,
        assign_lead_ms: Option<i32>,
        next_assign_ms: Option<i64>,
        enabled: bool,
    ) {
        let sql = format!(
            "insert into jobx_job (id, name, job_type, high_freq, cron, interval_duration, \
             assign_lead_ms, next_assign_ms, executor_code, enabled, creater_id, create_ms, \
             updater_id, update_ms) values ({id}, '{name}', {job_type}, {high_freq}, {}, {}, {}, {}, \
             'test-executor', {enabled}, 0, 0, 0, 0)",
            cron.map(|v| format!("'{v}'")).unwrap_or("null".to_string()),
            interval_duration
                .map(|v| format!("'{v}'"))
                .unwrap_or("null".to_string()),
            assign_lead_ms
                .map(|v| v.to_string())
                .unwrap_or("null".to_string()),
            next_assign_ms
                .map(|v| v.to_string())
                .unwrap_or("null".to_string()),
        );
        db.execute_unprepared(&sql).await.expect("insert 失败");
    }

    async fn load_job(db: &sea_orm::DatabaseConnection, id: u64) -> JobxJobVo {
        JobxJobDao::get_by_id::<_, JobxJobVo>(U64::from(id), db)
            .await
            .expect("查询失败")
            .expect("记录不存在")
    }

    /// 只拉回「启用 + 非 Manual + 滞后超阈值」的计划
    #[tokio::test]
    async fn revive_stale_jobs_only_touches_stale() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        let now = now_ms() as i64;
        let lag_ms = 3_600_000i64;
        // 1: 滞后 2 小时 → 应被拉回
        insert_job(&db, 1, "stale", 2, false, None, Some("5min"), Some(60_000), Some(now - 7_200_000), true).await;
        // 2: 滞后 10 分钟 → 不动
        insert_job(&db, 2, "fresh", 2, false, None, Some("5min"), Some(60_000), Some(now - 600_000), true).await;
        // 3: 滞后 2 小时但已停用 → 不动
        insert_job(&db, 3, "disabled", 2, false, None, Some("5min"), Some(60_000), Some(now - 7_200_000), false).await;
        // 4: 滞后 2 小时但 Manual → 不动
        insert_job(&db, 4, "manual", 0, false, None, None, None, Some(now - 7_200_000), true).await;
        // 5: next_assign_ms 为 NULL → 不动
        insert_job(&db, 5, "null-next", 2, false, None, Some("5min"), Some(60_000), None, true).await;

        let affected = JobxJobSvc::revive_stale_jobs(now, lag_ms, &db)
            .await
            .expect("拉回失败");

        assert_eq!(affected, 1, "只有 stale 这一条应当被拉回");
        assert_eq!(load_job(&db, 1).await.next_assign_ms.map(|v| v.value()), Some(now as u64));
        assert_eq!(
            load_job(&db, 2).await.next_assign_ms.map(|v| v.value()),
            Some((now - 600_000) as u64)
        );
        assert_eq!(
            load_job(&db, 3).await.next_assign_ms.map(|v| v.value()),
            Some((now - 7_200_000) as u64)
        );
        assert_eq!(
            load_job(&db, 4).await.next_assign_ms.map(|v| v.value()),
            Some((now - 7_200_000) as u64)
        );
        assert!(load_job(&db, 5).await.next_assign_ms.is_none());

        // 幂等：再跑一次不该再命中任何行
        let again = JobxJobSvc::revive_stale_jobs(now, lag_ms, &db)
            .await
            .expect("第二次拉回失败");
        assert_eq!(again, 0);
    }

    /// 非高频任务推进：`next_assign += interval`
    #[tokio::test]
    async fn recalc_advances_by_interval() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        let start = 1_800_000_000_000i64;
        insert_job(&db, 1, "fixed", 2, false, None, Some("5min"), Some(60_000), Some(start), true).await;

        JobxJobSvc::recalc_next_assign_ms(U64::from(1u64), Some(&db))
            .await
            .expect("推进失败");

        let job = load_job(&db, 1).await;
        assert_eq!(
            job.next_assign_ms.map(|v| v.value()),
            Some((start + 300_000) as u64),
            "非高频任务每次推进一个 interval"
        );
    }

    /// 高频任务推进：`next_assign += high_freq_assign_interval`，
    /// 于是相邻「预定执行时刻」的间距严格等于 `high_freq_assign_interval`，
    /// 也正好等于上一段 `exec_deadline_ms` 的位置（段与段首尾相接）
    #[tokio::test]
    async fn recalc_advances_high_freq_by_assign_interval() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        let start = 1_800_000_000_000i64;
        let lead = 60_000i64;
        let hf_interval = JobxSchedulerConfig::default()
            .high_freq_assign_interval
            .as_millis() as i64;
        insert_job(&db, 1, "hf", 2, true, None, Some("5min"), Some(lead as i32), Some(start), true).await;

        JobxJobSvc::recalc_next_assign_ms(U64::from(1u64), Some(&db))
            .await
            .expect("推进失败");

        let job = load_job(&db, 1).await;
        let next = job.next_assign_ms.map(|v| v.value()).unwrap() as i64;
        assert_eq!(
            next,
            start + hf_interval,
            "高频任务的 next_assign 增量应等于 high_freq_assign_interval"
        );
        // 真正有物理意义的是执行时刻：间距应当严格等于 hf_interval，
        // 也正好是上一段 exec_deadline_ms 的位置（scheduled + hf_interval）
        assert_eq!((next + lead) - (start + lead), hf_interval);
    }

    /// Cron 任务推进后仍落在 cron 拍点上
    #[tokio::test]
    async fn recalc_cron_stays_on_beat() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        let lead = 60_000i64;
        let start = Utc
            .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
            .unwrap()
            .timestamp_millis()
            - lead;
        insert_job(&db, 1, "cron", 1, false, Some("0 */5 * * * *"), None, Some(lead as i32), Some(start), true).await;

        JobxJobSvc::recalc_next_assign_ms(U64::from(1u64), Some(&db))
            .await
            .expect("推进失败");

        let job = load_job(&db, 1).await;
        let next = job.next_assign_ms.map(|v| v.value()).unwrap() as i64;
        assert!(next > start, "cron 应当向前推进");
        assert_eq!((next + lead) % 300_000, 0, "推进后仍应落在 5 分钟边界");
    }

    /// 只改备注：不重算 `next_assign_ms`（不产生调度副作用）
    #[tokio::test]
    async fn before_modify_skips_recompute_when_schedule_unchanged() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        let next_assign = 1_800_000_000_000i64;
        insert_job(&db, 1, "job", 2, false, None, Some("5min"), Some(60_000), Some(next_assign), true).await;

        let dto: JobxJobModifyDto =
            serde_json::from_value(json!({"id": "1", "remark": "只改备注"})).unwrap();
        let out = JobxJobSvc::before_modify(dto, &db).await.expect("钩子失败");

        assert!(
            out.next_assign_ms.is_none(),
            "调度参数没变时不应写 next_assign_ms（None = 不更新该列）"
        );
        assert!(out.high_freq.is_none());
    }

    /// 改了间隔：重算 `next_assign_ms`
    #[tokio::test]
    async fn before_modify_recomputes_when_interval_changed() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        insert_job(&db, 1, "job", 2, false, None, Some("5min"), Some(60_000), Some(1_800_000_000_000), true).await;

        // Duration 序列化为毫秒数
        let dto: JobxJobModifyDto =
            serde_json::from_value(json!({"id": "1", "intervalDuration": 600_000})).unwrap();
        let out = JobxJobSvc::before_modify(dto, &db).await.expect("钩子失败");

        let recomputed = out
            .next_assign_ms
            .expect("调度参数变了必须重算")
            .expect("非 Manual 计划必须有 next_assign_ms");
        // 重算基准是 now，因此应当落在当前时间附近而不是旧值上
        let now = now_ms();
        assert!(
            recomputed.value() > now.saturating_sub(3_600_000),
            "重算应把 next_assign_ms 拉回当前时间附近，实际 {}",
            recomputed.value()
        );
    }

    /// 切到 Manual：清空调度派生字段
    #[tokio::test]
    async fn before_modify_clears_when_switching_to_manual() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        insert_job(&db, 1, "job", 2, false, None, Some("5min"), Some(60_000), Some(1_800_000_000_000), true).await;

        let dto: JobxJobModifyDto =
            serde_json::from_value(json!({"id": "1", "jobType": 0})).unwrap();
        let out = JobxJobSvc::before_modify(dto, &db).await.expect("钩子失败");

        assert_eq!(out.next_assign_ms, Some(None), "Manual 计划必须清空 next_assign_ms");
        assert_eq!(out.high_freq, Some(None), "Manual 计划必须清空 high_freq");
        assert_eq!(out.assign_lead_ms, Some(None), "Manual 计划必须清空 assign_lead_ms");
    }

    /// 端到端：滞后到查询窗口之外的「停摆」计划，被拉回后重新可发布
    ///
    /// 这条断言直接覆盖风险本身：`next_assign_ms` 掉出
    /// `[now - query_start_earlier, now + query_end_later]` 之后，
    /// `list_publishable` 就再也选不到它——**没有任何日志**。
    #[tokio::test]
    async fn revive_makes_stale_job_publishable_again() {
        let _guard = DB_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init_scheduler_config();
        let Some(db) = connect_test_db().await else {
            return;
        };
        reset_table(&db).await;

        let cfg = JobxSchedulerConfig::default();
        let now = now_ms() as i64;
        let begin = now - cfg.query_start_earlier_duration.as_millis() as i64;
        let end = now + cfg.query_end_later_duration.as_millis() as i64;

        // 滞后 5 天，已经掉到查询窗口下界（now - 3 天）之外
        let five_days_ms = 5 * 24 * 3_600_000i64;
        insert_job(
            &db,
            1,
            "stalled",
            2,
            false,
            None,
            Some("5min"),
            Some(60_000),
            Some(now - five_days_ms),
            true,
        )
        .await;

        let before = JobxJobDao::list_publishable(begin, end, now, &db)
            .await
            .expect("查询失败");
        assert!(
            before.is_empty(),
            "滞后 5 天的计划不应出现在可发布列表里——这正是静默停摆"
        );

        let affected =
            JobxJobSvc::revive_stale_jobs(now, cfg.max_assign_lag_duration.as_millis() as i64, &db)
                .await
                .expect("拉回失败");
        assert_eq!(affected, 1);

        let after = JobxJobDao::list_publishable(begin, end, now, &db)
            .await
            .expect("查询失败");
        assert_eq!(after.len(), 1, "拉回后应当重新进入可发布列表");
        assert_eq!(after[0].id, U64::from(1u64));
    }
}
