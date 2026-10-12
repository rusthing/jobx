use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use typed_builder::TypedBuilder;
use wheel_rs::serde::{duration_option_serde, duration_serde};

/// Executor 配置
#[derive(Debug, Clone, Serialize, Deserialize, Setters, TypedBuilder)]
#[serde(rename_all = "kebab-case")]
pub struct JobxExecutorConfig {
    /// Redis Stream 的 key的前缀，后面跟着任务执行器的编码
    #[serde(default = "executor_key_default")]
    #[builder(default = executor_key_default())]
    pub executor_key: String,
    /// 任务执行器的编码
    pub executor_code: String,
    /// 任务执行器订阅消息分组
    #[serde(default = "executor_group_default")]
    #[builder(default = executor_group_default())]
    pub executor_group: Option<String>,
    /// 扫描间隔
    #[serde(with = "duration_serde", default = "scan_interval_default")]
    #[builder(default = scan_interval_default())]
    pub scan_interval: Duration,
    /// 阻塞超时时间
    /// Some(Duration::ZERO) 表示永久阻塞直到有新消息；
    //  Some(Duration::from_millis(N)) 表示最多等待 N 毫秒，超时返回空结果；
    //  None 表示非阻塞立即返回。
    #[serde(with = "duration_option_serde", default = "block_duration_default")]
    #[builder(default = block_duration_default())]
    pub block_duration: Option<Duration>,
    /// 单次接收消息的最大数量
    #[serde(default = "max_messages_default")]
    #[builder(default = max_messages_default())]
    pub max_messages: usize,
    /// 上报执行结果失败时最大重试次数
    #[serde(default = "report_retry_count_default")]
    #[builder(default = report_retry_count_default())]
    pub report_retry_count: u32,
    /// 上报执行结果重试间隔
    #[serde(with = "duration_serde", default = "report_retry_interval_default")]
    #[builder(default = report_retry_interval_default())]
    pub report_retry_interval: Duration,
}

fn executor_key_default() -> String {
    "jobx:executor".to_string()
}

fn executor_group_default() -> Option<String> {
    Some("Default".to_string())
}

fn scan_interval_default() -> Duration {
    Duration::from_secs(1)
}

fn block_duration_default() -> Option<Duration> {
    Some(Duration::from_secs(5))
}

fn max_messages_default() -> usize {
    5
}

fn report_retry_count_default() -> u32 {
    3
}

fn report_retry_interval_default() -> Duration {
    Duration::from_secs(1)
}
