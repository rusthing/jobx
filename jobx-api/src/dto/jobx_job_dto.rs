use crate::dic::JobType;
use robotech::api::Duration;
use robotech::macros::crud_dto;

#[crud_dto]
pub struct JobxJobDto {
    /// 执行器编码
    /// 发布任务消息时的key将以此编码结尾，只有相同编码的执行器才订阅此key
    /// @len 1..=50
    pub executor_code: String,
    /// 名称
    /// @len 1..=50
    pub name: String,
    /// 参数
    /// @len 0..=800
    pub params: Option<String>,
    /// 计划类型: 0=手动分派, 1=cron表达式, 2=固定延迟, 3=固定频率
    pub job_type: JobType,
    /// 是否高频任务
    /// 这个由计划类型、cron表达式、固定时间间隔与配置中的高频任务时长阈值计算得出
    /// 库默认 false，新增不传时由数据库补默认值
    #[db_default]
    pub high_freq: Option<bool>,
    /// cron表达式
    /// 当 job_type 为 cron表达式时必填
    /// @len 0..=30
    pub cron: Option<String>,
    /// 固定间隔时间
    /// 当job_type为固定延迟或固定频率时必填
    pub interval_duration: Option<Duration>,
    /// 有效开始时间戳
    pub valid_begin_ms: Option<u64>,
    /// 有效结束时间戳
    pub valid_end_ms: Option<u64>,
    /// 分派提前毫秒数
    pub assign_lead_ms: Option<u32>,
    /// 下次分派时间戳
    pub next_assign_ms: Option<u64>,
    /// Stream长度上限
    /// 发布该计划的消息时使用的 Stream 长度上限（MAXLEN ~ N）
    /// 为空时采用调度器的全局默认值（jobx-scheduler.stream-max-len，默认 10000）
    pub stream_max_len: Option<u32>,
    /// 任务保留时长
    ///
    /// 该计划产生的任务记录（`jobx_task`）超过这个时长后会被服务端自动清理。
    /// 基准是**任务的创建时刻**（`create_ms`），与任务是否执行完无关；
    /// 留空（null）表示永久保留，不参与清理。
    pub task_expire_duration: Option<Duration>,
    /// 备注
    /// @len 0..=50
    pub remark: Option<String>,
    /// 启用
    #[db_default]
    pub enabled: bool,
}

/// # 手动触发计划执行的请求参数
///
/// 字段名与其它 DTO 保持一致使用 camelCase（`job_id` ↔ `jobId`），前端无需为这一个接口
/// 特殊处理命名风格。ID 用 `U64` 而非 `u64`：雪花 ID 是 19 位，超过 JS 安全整数范围，
/// 按全仓约定以字符串传输，避免前端精度丢失。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobxJobTriggerDto {
    /// 任务计划ID
    pub job_id: robotech::api::U64,
}
