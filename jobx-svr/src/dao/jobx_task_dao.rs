use robotech::macros::dao;

/// 任务记录
#[dao(
    foreign_keys: [
        ("job_id", "jobx_job", "任务计划"),
    ],
    unique_keys: [
        ("job_id,scheduled_ts", "任务计划与预定时间戳"),
    ],
)]
pub struct JobxTaskDao;