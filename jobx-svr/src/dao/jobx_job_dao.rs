use jobx_api::dic::JobType;
use jobx_api::mo::jobx_task;
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
    pub async fn find_publishable<C>(
        begin: i64,
        end: i64,
        now: i64,
        db: &C,
    ) -> Result<Vec<Model>, DaoError>
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
                Expr::col(jobx_task::Column::ScheduledAssignMs),
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
            .all(db)
            .await
            .map_err(|e| DaoError::parse_db_err(e))
    }
}