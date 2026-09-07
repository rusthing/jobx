use crate::config::jobx_config::JobxConfig;
use idworker::IdWorkerConfig;
use robotech::db::DbConnConfig;
use robotech::web::WebServerConfig;
use serde::Deserialize;

/// 配置文件结构
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    /// jobx
    #[serde(default = "JobxConfig::default")]
    pub jobx: JobxConfig,
    /// db
    pub db: DbConnConfig,
    /// Web服务器
    #[serde(default = "WebServerConfig::default")]
    pub web: WebServerConfig,
    /// id_worker
    #[serde(default = "IdWorkerConfig::default")]
    pub id_worker: IdWorkerConfig,
}
