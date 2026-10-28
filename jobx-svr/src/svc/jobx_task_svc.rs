use crate::config::{JobxSchedulerConfig, get_jobx_scheduler_config};
use crate::svc::JobxJobSvc;
use anyhow::anyhow;
use jobx_api::dic::TaskStatus;
use jobx_api::mo::{jobx_job, jobx_task};
use jobx_api::vo::JobxJobVo;
use robotech::cst::user_id_cst::SYS_OPERATOR_USER_ID;
use robotech::macros::svc;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, Condition, DbBackend, EntityTrait, QueryFilter, Statement};
use tracing::warn;
use wheel_rs::time_utils::now_ms;

#[svc(before_add = Self::before_add)]
pub struct JobxTaskSvc;

impl JobxTaskSvc {
    /// # add 前置钩子：给高频任务算出「本段执行截止时刻」
    ///
    /// 高频任务的一次分派代表**一段**连续执行：服务端 `recalc_next_assign_ms` 按
    /// `high_freq_assign_interval`（默认 5 分钟）推进下一次分派，因此这段执行的天然
    /// 终点就是「预定开始执行时刻 + 高频分派间隔」。
    ///
    /// ## 为什么必须由服务端算
    ///
    /// `high_freq_assign_interval` 是调度器配置，只在 jobx-svr 里；执行器（jobx-wkr）
    /// 拿到的 `JobxTaskVo` 里没有这个值，它只能退而用计划整体的 `valid_end_ms` 当终点。
    /// 而 `valid_end_ms` 是「计划有效期」，为空（很常见）时执行器会把循环跑到
    /// `u64::MAX` —— 任务永远不结束、永远挂在「运行中」。
    ///
    /// ## 计算式
    ///
    /// `min(valid_end_ms, scheduled_exec_start_ms + high_freq_assign_interval)`
    ///
    /// 起点优先用预定执行时刻；立即执行的任务（手动触发）没有预定时刻，
    /// 退化为 `assign_ms`（即创建时刻）。
    ///
    /// 非高频任务不写这个字段：它们的执行是一次性的，结束条件由执行器自己掌握。
    ///
    /// 纯计算部分抽到 [`Self::calc_exec_deadline`]，便于单测覆盖边界。
    async fn before_add<C: ConnectionTrait>(
        mut add_dto: JobxTaskAddDto,
        _db: &C,
    ) -> Result<JobxTaskAddDto, SvcError> {
        let is_high_freq = add_dto.high_freq.flatten().unwrap_or(false);
        if is_high_freq {
            let JobxSchedulerConfig {
                high_freq_assign_interval,
                ..
            } = *get_jobx_scheduler_config()?;

            let start_ms = add_dto
                .scheduled_exec_start_ms
                .flatten()
                .map(|v| v.value())
                .or_else(|| add_dto.assign_ms.as_ref().map(|v| v.value()))
                .unwrap_or_else(now_ms);
            let deadline_ms = Self::calc_exec_deadline(
                start_ms,
                add_dto.valid_end_ms.flatten().map(|v| v.value()),
                high_freq_assign_interval.as_millis() as u64,
            );

            add_dto.exec_deadline_ms = Some(Some(U64::from(deadline_ms)));
        }
        Ok(add_dto)
    }

    /// # 计算高频任务本段的执行截止时刻
    ///
    /// `min(valid_end_ms, start_ms + hf_interval_ms)`，`valid_end_ms` 为空表示不设上限。
    ///
    /// 用饱和加法：`start_ms` 接近 `u64::MAX`（手工触发传了极端值）时不能溢出。
    /// 计划有效期更早时以它为准——那之后本来就不该再执行。
    fn calc_exec_deadline(start_ms: u64, valid_end_ms: Option<u64>, hf_interval_ms: u64) -> u64 {
        let segment_end_ms = start_ms.saturating_add(hf_interval_ms);
        valid_end_ms.map_or(segment_end_ms, |valid_end| segment_end_ms.min(valid_end))
    }
}

impl JobxTaskSvc {
    /// # Worker 领取任务（在同一事务中创建 task 并推进 job 的下次分派时间）
    ///
    /// 由 Worker 通过 `/jobx/task/take` 接口调用。在一个数据库事务中完成：
    /// 1. 根据 `add_dto` 创建一条任务记录（通过自动生成的 `add` 方法）
    /// 2. 若任务关联了任务计划（`job_id` 不为空），推进该计划的下次分派时间
    ///
    /// 这样做保证任务创建与调度推进的原子性，避免任务已创建但调度未更新的不一致状态。
    ///
    /// 高频任务的「本段执行截止时刻」也在这里补齐（由 `before_add` 钩子写入
    /// `exec_deadline_ms`），执行器据此结束循环，不再依赖计划整体的 `valid_end_ms`。
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

    /// # 回收僵尸任务
    ///
    /// 把「服务端已建单、执行器再也不会来上报」的记录收敛成 `Failed`，分三条规则：
    ///
    /// 1. **超时未开始**：`status = Running` 且 `exec_start_ms is null` 且
    ///    `scheduled_exec_start_ms < now - start_grace`。
    ///    执行器 `take` 之后就落了一条 Running 记录，随后才等预定执行时刻；
    ///    进程在这个窗口里死掉，记录就永远停在这里。
    /// 2. **执行超时（普通任务）**：已开始执行（`exec_start_ms` 非空）但
    ///    `exec_start_ms < now - exec_timeout`，且不是高频任务。
    /// 3. **执行超时（高频任务）**：同上，但阈值换成 `high_freq_exec_timeout`。
    ///    高频任务本来就长时间连续执行，用普通阈值会全部误杀。
    ///
    /// ## 为什么必须由服务端做
    ///
    /// 消息在 `take` 成功那一刻就已经 ACK，Redis PEL 里没有残留，`XAUTOCLAIM`
    /// 认领不到任何东西；「崩溃恢复的粒度在 `jobx_task` 行上」，而这一行此前
    /// **没有任何超时判定**，只能由服务端自己扫。
    ///
    /// ## 幂等与并发
    ///
    /// 三条 UPDATE 都带 `status = Running and exec_end_ms is null` 条件，
    /// 多副本同时巡检时只有第一个能命中（`rows_affected` 反映真实回收条数），
    /// 不会重复改写已收敛的记录。
    ///
    /// ## 误判可自愈
    ///
    /// 被误判成超时的任务，执行器跑完后仍会正常上报，状态会被改回真实结果
    /// （后写覆盖）。所以阈值宁可放宽。
    ///
    /// ## 参数
    ///
    /// * `now` - 当前时间戳（毫秒），由调用方传入以便一次巡检内各规则共用同一基准
    /// * `start_grace_ms` - 「已分派未开始」的宽限期（毫秒）
    /// * `exec_timeout_ms` - 普通任务的执行超时（毫秒）
    /// * `high_freq_exec_timeout_ms` - 高频任务的执行超时（毫秒）
    /// * `db` - 数据库连接（事务）
    #[db_unwrap]
    #[log_call]
    pub async fn reap_zombie_tasks<C>(
        now: i64,
        start_grace_ms: i64,
        exec_timeout_ms: i64,
        high_freq_exec_timeout_ms: i64,
        #[skip_log] db: Option<&C>,
    ) -> Result<ZombieReapResult, SvcError>
    where
        C: ConnectionTrait,
    {
        let updater_id = SYS_OPERATOR_USER_ID as i64;
        // 三条规则只差过滤条件与明细文案，收尾动作完全一致：置 Failed、补 exec_end_ms、
        // 写明细、记系统操作者。用闭包复用，避免三份「改了状态却忘了改明细」的漂移。
        let close = |detail: String| {
            jobx_task::Entity::update_many()
                .col_expr(
                    jobx_task::Column::Status,
                    Expr::value(TaskStatus::Failed.value()),
                )
                .col_expr(jobx_task::Column::ExecEndMs, Expr::value(now))
                .col_expr(jobx_task::Column::ExecDetail, Expr::value(detail))
                .col_expr(jobx_task::Column::UpdaterId, Expr::value(updater_id))
                .col_expr(jobx_task::Column::UpdateMs, Expr::value(now))
                .filter(jobx_task::Column::Status.eq(TaskStatus::Running.value()))
                .filter(jobx_task::Column::ExecEndMs.is_null())
        };

        // ---- 规则 1：已分派但超时未开始 ----
        let not_started = close(format!(
            "执行器失联：已分派但超过 {}s 仍未开始执行，由服务端巡检回收",
            start_grace_ms / 1000
        ))
        .filter(jobx_task::Column::ExecStartMs.is_null())
        .filter(jobx_task::Column::ScheduledExecStartMs.lt(now - start_grace_ms))
        .exec(db)
        .await
        .map_err(|e| anyhow!("回收「未开始」僵尸任务失败: {e}"))?
        .rows_affected;

        // ---- 规则 2：普通任务执行超时 ----
        // 高频标记可能为 NULL（历史数据），按「非高频」处理
        let timed_out = close(format!(
            "执行超时：已开始执行超过 {}s 仍未上报结果，由服务端巡检回收",
            exec_timeout_ms / 1000
        ))
        .filter(jobx_task::Column::ExecStartMs.lt(now - exec_timeout_ms))
        .filter(
            Condition::any()
                .add(jobx_task::Column::HighFreq.is_null())
                .add(jobx_task::Column::HighFreq.eq(false)),
        )
        .exec(db)
        .await
        .map_err(|e| anyhow!("回收「执行超时」僵尸任务失败: {e}"))?
        .rows_affected;

        // ---- 规则 3：高频任务执行超时 ----
        let high_freq_timed_out = close(format!(
            "执行超时：高频任务已连续执行超过 {}s 仍未上报结果，由服务端巡检回收",
            high_freq_exec_timeout_ms / 1000
        ))
        .filter(jobx_task::Column::ExecStartMs.lt(now - high_freq_exec_timeout_ms))
        .filter(jobx_task::Column::HighFreq.eq(true))
        .exec(db)
        .await
        .map_err(|e| anyhow!("回收「高频执行超时」僵尸任务失败: {e}"))?
        .rows_affected;

        Ok(ZombieReapResult {
            not_started,
            timed_out,
            high_freq_timed_out,
        })
    }

    /// # 清理过期任务
    ///
    /// 按**计划级**的保留时长（`jobx_job.task_expire_duration`）删除超期的
    /// `jobx_task` 记录。保留时长为 `NULL` 的计划不参与清理。
    ///
    /// ## 过期基准是 `create_ms`
    ///
    /// 判定条件是 `create_ms < now - 保留时长`。用创建时刻而不是 `scheduled_exec_start_ms`
    /// 或 `exec_end_ms`：
    ///
    /// - `scheduled_exec_start_ms` 可以被计划改期推到很远，改完之后老记录会被「续命」；
    /// - `exec_end_ms` 在任务结束前为空，等于「跑不完就永远不清理」，而卡住的记录
    ///   正是最该被清掉的那批。
    ///
    /// `create_ms` 是 NOT NULL、写入后不变，且与 `(job_id, create_ms)` 索引配合最好。
    ///
    /// ## 不区分任务状态
    ///
    /// `Running` 的超期记录同样会被删。设计上「卡住」由 `jobx-task-reaper` 在分钟级
    /// 收敛（阈值远小于任何合理的保留时长），等它超期时早已不是 `Running`。
    /// 真删到了在跑的任务，执行器事后上报只会命中 0 行，不影响执行器本身。
    ///
    /// ## 分批与上限
    ///
    /// 每个计划每轮最多删 `max_batches_per_job` 批、每批 `batch_size` 行，用
    /// `delete ... where id in (select id ... limit N)` 实现：一次删几十万行会让事务
    /// 长时间持锁并打出巨量 WAL，反过来挡住执行器写入新任务记录。
    ///
    /// 达到上限时把该计划计入 [`ExpiredTaskCleanResult::truncated`]，下一轮继续。
    ///
    /// ## 幂等与并发
    ///
    /// DELETE 天然幂等，多副本同时清理最多是重复扫描：后到的 DELETE 命中 0 行。
    ///
    /// ## 参数
    ///
    /// * `now` - 当前时间戳（毫秒），由调用方传入以便一轮清理内共用同一基准
    /// * `batch_size` - 单批删除行数（0 按 1 处理）
    /// * `max_batches_per_job` - 单计划每轮最多删几批（0 按 1 处理）
    /// * `db` - 数据库连接（事务）
    #[db_unwrap]
    #[log_call]
    pub async fn clean_expired_tasks<C>(
        now: i64,
        batch_size: u64,
        max_batches_per_job: u32,
        #[skip_log] db: Option<&C>,
    ) -> Result<ExpiredTaskCleanResult, SvcError>
    where
        C: ConnectionTrait,
    {
        // 只捞设了保留时长的计划：绝大多数计划是 NULL（永久保留），
        // 在 SQL 里过滤掉，避免每轮把整张 jobx_job 拉回来。
        let jobs = jobx_job::Entity::find()
            .filter(jobx_job::Column::TaskExpireDuration.is_not_null())
            .into_model::<JobxJobVo>()
            .all(db)
            .await
            .map_err(|e| anyhow!("查询待清理的任务计划失败: {e}"))?;

        let batch_size = batch_size.max(1);
        let max_batches = max_batches_per_job.max(1);
        let mut result = ExpiredTaskCleanResult::default();

        for job in jobs {
            // 查询条件已经保证非空；let-else 是为了将来放宽查询条件时不至于 panic
            let Some(expire_duration) = job.task_expire_duration else {
                continue;
            };
            // u128 -> i64 饱和转换：极端值（几百年）不该让整轮清理失败
            let expire_ms = i64::try_from(expire_duration.as_millis()).unwrap_or(i64::MAX);
            if expire_ms <= 0 {
                // 0 时长等于「创建即过期」，会把还在跑的任务记录立刻删掉。
                // 这种配置几乎肯定是误填（例如前端把空值当 0 传上来），跳过并告警。
                warn!(
                    "计划 {} ({}) 的任务保留时长为 0，跳过清理（如需永久保留请留空）",
                    job.name,
                    job.id.value()
                );
                continue;
            }

            let job_id = job.id.value() as i64;
            let threshold = now.saturating_sub(expire_ms);

            // 用 id 而不是 ctid 做分批：id 是主键，SQL 也不绑定 PostgreSQL 专有语法
            let sql = "delete from jobx_task where id in ( \
                       select id from jobx_task where job_id = $1 and create_ms < $2 limit $3 \
                       )";
            let mut deleted = 0u64;
            let mut drained = false;
            for _ in 0..max_batches {
                let statement = Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql,
                    [job_id.into(), threshold.into(), (batch_size as i64).into()],
                );
                let affected = db
                    .execute_raw(statement)
                    .await
                    .map_err(|e| anyhow!("清理计划 {job_id} 的过期任务失败: {e}"))?
                    .rows_affected();
                deleted += affected;
                // 这一批没删满，说明已删干净，不用再试
                if affected < batch_size {
                    drained = true;
                    break;
                }
            }

            if deleted > 0 {
                result.jobs += 1;
                result.deleted += deleted;
            }
            // 批次数用尽且最后一批仍是满的：大概率还有剩余，留给下一轮
            if !drained {
                result.truncated += 1;
            }
        }

        Ok(result)
    }
}

/// # 过期任务清理结果
///
/// 三个计数分开报，是为了让日志能区分「正常清理」和「积压没清完」：
/// 只有 `truncated > 0` 才需要人去调 `batch-size` / `max-batches-per-job`。
#[derive(Debug, Default, Clone, Copy)]
pub struct ExpiredTaskCleanResult {
    /// 本轮真正删到行的计划数
    pub jobs: u64,
    /// 本轮删除的任务记录行数
    pub deleted: u64,
    /// 达到批次数上限、大概率仍有剩余的计划数
    pub truncated: u64,
}

impl ExpiredTaskCleanResult {
    /// 本轮是否删到了东西（用于决定要不要打日志）
    pub fn total(&self) -> u64 {
        self.deleted
    }
}

/// # 僵尸任务回收结果
///
/// 三条规则各自命中的条数。分开统计是为了让日志能区分「执行器根本没跑」
/// 和「跑了但没上报」—— 前者多半是发布/重启，后者多半是上报链路或 handler 卡死。
#[derive(Debug, Default, Clone, Copy)]
pub struct ZombieReapResult {
    /// 已分派但超过宽限期仍未开始执行
    pub not_started: u64,
    /// 普通任务执行超时
    pub timed_out: u64,
    /// 高频任务执行超时
    pub high_freq_timed_out: u64,
}

impl ZombieReapResult {
    /// 本次回收总数
    pub fn total(&self) -> u64 {
        self.not_started + self.timed_out + self.high_freq_timed_out
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const HF: u64 = 300_000;

    /// 无有效期上限：截止 = 起点 + 高频间隔
    #[test]
    fn deadline_is_start_plus_interval() {
        assert_eq!(JobxTaskSvc::calc_exec_deadline(1_000, None, HF), 301_000);
    }

    /// 计划有效期更早：以有效期为准
    #[test]
    fn deadline_respects_earlier_valid_end() {
        assert_eq!(
            JobxTaskSvc::calc_exec_deadline(1_000, Some(200_000), HF),
            200_000
        );
    }

    /// 计划有效期更晚：仍取本段终点
    #[test]
    fn deadline_ignores_later_valid_end() {
        assert_eq!(
            JobxTaskSvc::calc_exec_deadline(1_000, Some(999_999), HF),
            301_000
        );
    }

    /// 起点接近 u64::MAX 时饱和，不溢出
    #[test]
    fn deadline_saturates_on_overflow() {
        assert_eq!(
            JobxTaskSvc::calc_exec_deadline(u64::MAX - 10, None, HF),
            u64::MAX
        );
    }
}
