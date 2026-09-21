use robotech::macros::dao;

/// 任务记录
///
/// ## 外键
/// - `job_id` → `jobx_job.id`：关联任务计划，删除/更新受限
///
/// ## 唯一约束
/// - `(job_id, scheduled_exec_start_ms)`：同一任务计划下预定执行开始时间唯一，
///   确保一个任务计划在同一预定执行时间点不会产生重复的任务记录
#[dao(
    foreign_keys: [
        ("job_id", "jobx_job", "任务计划"),
    ],
    unique_keys: [
        ("job_id,scheduled_exec_start_ms", "任务计划与预定执行开始时间戳"),
    ],
)]
pub struct JobxTaskDao;