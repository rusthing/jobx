use crate::config::{JobxSchedulerConfig, get_jobx_scheduler_config};
use crate::svc::JobxJobSvc;
use anyhow::anyhow;
use jobx_api::dic::TaskStatus;
use jobx_api::mo::jobx_task;
use robotech::cst::user_id_cst::SYS_OPERATOR_USER_ID;
use robotech::macros::svc;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
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
