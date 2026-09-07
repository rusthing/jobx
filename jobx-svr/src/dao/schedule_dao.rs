use robotech::macros::dao;

#[dao(
    like_columns: [
        Column::Cron,
        Column::Remark,
    ],
)]
pub struct ScheduleDao;
