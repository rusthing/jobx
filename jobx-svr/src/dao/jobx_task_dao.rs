use robotech::macros::dao;

/// 任务记录
#[dao(
    foreign_keys: [
        ("job_id", "jobx_job", "任务计划"),
    ],
)]
pub struct JobxTaskDao;