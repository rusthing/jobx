use jobx_api::dic::JobType;
use robotech::macros::dao;
use sea_orm::ColumnTrait;

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
        Entity::find()
            .filter(Column::Enabled.eq(true))
            .filter(Column::JobType.ne(JobType::Manual.value()))
            .filter(Column::NextAssignTs.gte(begin))
            .filter(Column::NextAssignTs.lte(end))
            .all(db)
            .await
            .map_err(|e| DaoError::parse_db_err(e))
    }
}
