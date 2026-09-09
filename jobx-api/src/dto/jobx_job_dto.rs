use robotech::macros::crud_dto;

#[crud_dto]
pub struct JobxJobDto {
    /// 编码
    pub code: String,
    /// 名称
    pub name: String,
    /// 参数
    pub params: Option<String>,
    /// 计划类型: 1=cron表达式, 2=固定延迟, 3=固定频率
    pub scheduling_type: i16,
    /// cron表达式
    pub cron: Option<String>,
    /// 固定间隔时间
    pub interval_duration: Option<String>,
    /// 有效开始时间戳
    pub valid_begin_ts: Option<i64>,
    /// 有效结束时间戳
    pub valid_end_ts: Option<i64>,
    /// 下次触发时间戳
    pub next_trigger_ts: Option<i64>,
    /// 备注
    pub remark: Option<String>,
    /// 启用
    pub enabled: bool,
}