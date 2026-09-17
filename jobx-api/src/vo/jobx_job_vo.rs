use crate::dic::JobType;
use robotech::api::Duration;
use robotech::macros::vo;

#[vo]
pub struct JobxJobVo {
    /// ID
    pub id: u64,
    /// 编码
    /// 发布任务消息时的key将以计划编码结尾，对应的执行器订阅此key
    pub code: String,
    /// 名称
    pub name: String,
    /// 参数
    pub params: Option<String>,
    /// 计划类型: 0=手动分派, 1=cron表达式, 2=固定延迟, 3=固定频率
    pub job_type: JobType,
    /// 是否高频任务
    pub high_freq: bool,
    /// cron表达式
    pub cron: Option<String>,
    /// 固定间隔时间
    pub interval_duration: Option<Duration>,
    /// 有效开始时间戳
    pub valid_begin_ts: Option<u64>,
    /// 有效结束时间戳
    pub valid_end_ts: Option<u64>,
    /// 提前分派时间
    pub pre_assign_duration: Option<Duration>,
    /// 下次分派时间戳
    pub next_assign_ts: u64,
    /// 备注
    pub remark: Option<String>,
    /// 启用
    pub enabled: bool,
    /// 创建者ID
    pub creator_id: u64,
    /// 创建时间
    pub create_ts: u64,
    /// 更新者ID
    pub updator_id: u64,
    /// 更新时间
    pub update_ts: u64,
}
