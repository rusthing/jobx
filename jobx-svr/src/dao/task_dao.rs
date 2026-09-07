use robotech::macros::dao;

#[dao(
    like_columns: [
        Column::Status,
        Column::Result,
        Column::Remark,
    ],
)]
pub struct TaskDao;
