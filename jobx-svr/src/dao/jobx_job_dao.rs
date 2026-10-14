use jobx_api::dic::JobType;
use jobx_api::mo::jobx_task;
use jobx_api::vo::JobxJobVo;
use robotech::macros::dao;
use sea_orm::sea_query::{BinOper, Expr, Query, SimpleExpr};
use sea_orm::ColumnTrait;
use sea_orm::ExprTrait;

/// 任务计划
#[dao(
    unique_keys: [
        ("name", "名称"),
    ],
    like_columns: [
        Column::ExecutorCode,
        Column::Name,
        Column::Remark,
    ],
)]
pub struct JobxJobDao;

impl JobxJobDao {
    /// # 查询可发布的任务计划
    ///
    /// 在指定的时间窗口内查找已启用、非 Manual 类型且 `next_assign_ms` 落在 `[begin, end]`
    /// 范围内的任务计划。同时通过 NOT EXISTS 子查询排除已生成过任务的记录，确保幂等性：
    /// 若某个 job 的同一分派时刻已经生成了对应的 task 记录，则不再重复发布。
    ///
    /// ## 参数
    ///
    /// * `begin` - 分派时间窗口起始时间戳
    /// * `end` - 分派时间窗口结束时间戳
    /// * `now` - 当前时间戳，用于校验 `valid_begin_ms <= now <= valid_end_ms`
    /// * `db` - 数据库连接
    ///
    /// ## 子查询的两个易错点
    ///
    /// **1. 两侧列都必须带表名。** 子查询里的列用不带限定的 `Expr::col(...)` 写会生成
    /// `"job_id" = "id"` 这样的 SQL，而 PostgreSQL 解析未限定列名时**先看最内层作用域**：
    /// `"id"` 命中内层 `jobx_task.id`（不是外层 `jobx_job.id`），条件就变成
    /// 「任务主键 = 它自己的 job_id」——恒不成立，子查询恒为空，`NOT EXISTS` 恒为真，
    /// **去重彻底失效**（等价于根本没写这个条件）。所以这里统一用 `as_column_ref()`
    /// 生成 `("表名", "列名")` 的限定引用。
    ///
    /// **2. 比较的必须是「执行时刻」，不是「分派时刻」。** `jobx_task.scheduled_exec_start_ms`
    /// 存的是任务的**执行时刻**，而 `jobx_job.next_assign_ms` 存的是**分派时刻**，
    /// 两者相差一个分派提前量 —— 见 worker 的 `build_task_add_dto`：
    /// `scheduled_exec_start_ms = next_assign_ms + assign_lead_ms`。
    /// 因此右边必须写成 `next_assign_ms + assign_lead_ms`。
    ///
    /// 若图省事只补表限定、仍拿 `next_assign_ms` 直接比，当
    /// `assign_lead_ms == interval_duration`（本仓 dbz 健康检查正是这个配置：
    /// 间隔 60s、提前量 60s）时，执行时刻恰好等于下一个 `next_assign_ms`，
    /// 条件会**恒成立**、计划被永久挡住发不出去 —— 那就从「没去重」变成「死锁」了。
    ///
    /// 另外 `assign_lead_ms` 为 NULL 时右侧整体为 NULL，比较结果也是 NULL（不成立），
    /// 即该计划退化为「不去重」而不是「发不出去」。这个方向是安全的，
    /// 且实际上非 Manual 计划在 `before_add` 里必定会写入提前量，不会为 NULL。
    pub async fn list_publishable<C>(
        begin: i64,
        end: i64,
        now: i64,
        db: &C,
    ) -> Result<Vec<JobxJobVo>, DaoError>
    where
        C: ConnectionTrait,
    {
        let not_exists = Query::select()
            .expr(Expr::value(1))
            .from(jobx_task::Entity)
            .and_where(Expr::binary(
                Expr::col(jobx_task::Column::JobId.as_column_ref()),
                BinOper::Equal,
                Expr::col(Column::Id.as_column_ref()),
            ))
            .and_where(Expr::binary(
                Expr::col(jobx_task::Column::ScheduledExecStartMs.as_column_ref()),
                BinOper::Equal,
                Expr::col(Column::NextAssignMs.as_column_ref())
                    .add(Expr::col(Column::AssignLeadMs.as_column_ref())),
            ))
            .to_owned();

        Entity::find()
            .filter(Column::Enabled.eq(true))
            .filter(Column::JobType.ne(JobType::Manual.value()))
            .filter(Column::NextAssignMs.gte(begin))
            .filter(Column::NextAssignMs.lte(end))
            .filter(
                Condition::any()
                    .add(Column::ValidBeginMs.is_null())
                    .add(Column::ValidBeginMs.lte(now)),
            )
            .filter(
                Condition::any()
                    .add(Column::ValidEndMs.is_null())
                    .add(Column::ValidEndMs.gte(now)),
            )
            .filter(SimpleExpr::from(Expr::exists(not_exists).not()))
            .into_model::<JobxJobVo>()
            .all(db)
            .await
            .map_err(|e| DaoError::parse_db_err(e))
    }
}