use robotech::macros::dao;

/// 任务记录
///
/// ## 外键
/// - `job_id` → `jobx_job.id`：关联任务计划，删除/更新受限
///
/// ## 唯一约束
/// - `(job_id, scheduled_exec_start_ms)`：同一任务计划下预定执行开始时间唯一，
///   确保一个任务计划在同一预定执行时间点不会产生重复的任务记录
///
/// ## 模糊匹配
/// 只开 `executor_code`：执行列表的关键字搜索按执行器编码（`_keyword`），
/// 不把 `exec_detail` 这种长文本放进 LIKE —— 大表上对长文本做 `%...%`
/// 既走不了索引，也会把每页查询放大成顺序扫描。需要按执行 ID 精确查时，
/// 前端直接把数字当 `id` 条件传，不依赖 LIKE。
#[dao(
    foreign_keys: [
        ("job_id", "jobx_job", "任务计划"),
    ],
    unique_keys: [
        ("job_id,scheduled_exec_start_ms", "任务计划与预定执行开始时间戳"),
    ],
    like_columns: [
        Column::ExecutorCode,
    ],
)]
pub struct JobxTaskDao;