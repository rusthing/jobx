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
    /// 若某个 job 的同一 `next_assign_ms` 时间点已经生成了对应的 task 记录，则不再重复发布。
    ///
    /// ## 参数
    ///
    /// * `begin` - 分派时间窗口起始时间戳
    /// * `end` - 分派时间窗口结束时间戳
    /// * `now` - 当前时间戳，用于校验 `valid_begin_ms <= now <= valid_end_ms`
    /// * `db` - 数据库连接
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
                Expr::col(jobx_task::Column::JobId),
                BinOper::Equal,
                Expr::col(Column::Id),
            ))
            .and_where(Expr::binary(
                Expr::col(jobx_task::Column::ScheduledExecStartMs),
                BinOper::Equal,
                Expr::col(Column::NextAssignMs),
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