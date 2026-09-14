use jobx_api::dic::JobType;
use jobx_api::mo::jobx_task;
use robotech::macros::dao;
use sea_orm::sea_query::{BinOper, Expr, Query, SimpleExpr};
use sea_orm::ColumnTrait;
use sea_orm::ExprTrait;

/// 任务计划
#[dao(
    unique_keys: [
        ("code", "编码"),
        ("name", "名称"),
    ],
    like_columns: [
        Column::Code,
        Column::Name,
        Column::Remark,
    ],
)]
pub struct JobxJobDao;

impl JobxJobDao {
    pub async fn find_publishable<C>(begin: i64, end: i64, db: &C) -> Result<Vec<Model>, DaoError>
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
                Expr::col(jobx_task::Column::ScheduledAssignTs),
                BinOper::Equal,
                Expr::col(Column::NextAssignTs),
            ))
            .to_owned();

        Entity::find()
            .filter(Column::Enabled.eq(true))
            .filter(Column::JobType.ne(JobType::Manual.value()))
            .filter(Column::NextAssignTs.gte(begin))
            .filter(Column::NextAssignTs.lte(end))
            .filter(SimpleExpr::from(Expr::exists(not_exists).not()))
            .all(db)
            .await
            .map_err(|e| DaoError::parse_db_err(e))
    }
}