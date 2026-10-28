use crate::config::{JobxSchedulerConfig, JobxTaskReaperConfig};
use idworker::IdWorkerConfig;
use robotech::db::DbConnConfig;
use robotech::redis::RedisConfig;
use robotech::web::WebServerConfig;
use serde::Deserialize;

/// 配置文件结构
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    /// jobx_scheduler
    #[serde(default = "JobxSchedulerConfig::default")]
    pub jobx_scheduler: JobxSchedulerConfig,
    /// jobx_task_reaper（僵尸任务巡检）
    #[serde(default = "JobxTaskReaperConfig::default")]
    pub jobx_task_reaper: JobxTaskReaperConfig,
    /// db
    pub db: DbConnConfig,
    /// Web服务器
    #[serde(default = "WebServerConfig::default")]
    pub web: WebServerConfig,
    /// id_worker
    #[serde(default = "IdWorkerConfig::default")]
    pub id_worker: IdWorkerConfig,
    /// redis
    pub redis: RedisConfig,
}
