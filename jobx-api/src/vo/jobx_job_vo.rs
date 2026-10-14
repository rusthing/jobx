use crate::dic::JobType;
use robotech::api::Duration;
use robotech::macros::vo;

#[vo]
pub struct JobxJobVo {
    /// ID
    pub id: u64,
    /// 执行器编码
    /// 发布任务消息时的key将以此编码结尾，只有相同编码的执行器才订阅此key
    pub executor_code: String,
    /// 名称
    pub name: String,
    /// 参数
    pub params: Option<String>,
    /// 计划类型: 0=手动分派, 1=cron表达式, 2=固定延迟, 3=固定频率
    pub job_type: JobType,
    /// 是否高频任务
    /// 这个由计划类型、cron表达式、固定时间间隔与配置中的高频任务时长阈值计算得出
    pub high_freq: Option<bool>,
    /// cron表达式
    /// 当 job_type 为 cron表达式时必填
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
    /// 备注
    pub remark: Option<String>,
    /// 启用
    pub enabled: bool,
    /// 创建者ID
    pub creater_id: u64,
    /// 创建时间戳
    pub create_ms: u64,
    /// 更新者ID
    pub updater_id: u64,
    /// 更新时间戳
    pub update_ms: u64,
}
