use crate::config::{JobxSchedulerConfig, JobxTaskCleanerConfig, JobxTaskReaperConfig};
use arc_swap::ArcSwapOption;
use idworker::IdWorkerConfig;
use robotech::db::DbConnConfig;
use robotech::redis::RedisConfig;
use robotech::web::WebServerConfig;
use serde::Deserialize;
use std::sync::Arc;

/// 未显式声明运行环境时按 `prod` 处理：没有配置就按最严格的口径跑。
pub const DEFAULT_PROFILE: &str = "prod";

/// 受支持的运行环境。
///
/// 写错（`APP_PROFILE=deve`）时不能静默落回 prod 或悄悄当成 dev ——
/// 前者会让本地开发莫名要求登录，后者更危险：拼错一个字母就把免登录打开了。
pub const ALLOWED_PROFILES: [&str; 3] = ["dev", "test", "prod"];

/// 运行环境在进程内的缓存（配置热更新时整体替换）
static APP_PROFILE: ArcSwapOption<String> = ArcSwapOption::const_empty();

/// # 缓存运行环境
///
/// 由 `main` 在启动/热更新时调用；未初始化时按 [`DEFAULT_PROFILE`] 处理。
pub fn set_app_profile(profile: Option<&str>) {
    APP_PROFILE.store(Some(Arc::new(
        profile.unwrap_or(DEFAULT_PROFILE).trim().to_string(),
    )));
}

/// # 当前运行环境
pub fn profile_name() -> String {
    APP_PROFILE
        .load_full()
        .map(|profile| (*profile).clone())
        .unwrap_or_else(|| DEFAULT_PROFILE.to_string())
}

/// # 当前是否开发环境
///
/// 只有 `profile = dev` 才免登录；鉴权中间件与启动校验共用这一判定。
pub fn is_dev_profile() -> bool {
    profile_name() == "dev"
}

/// 配置文件结构
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    /// 运行环境（dev / test / prod）
    ///
    /// 由配置文件根节点的 `profile` 读取，也可以用环境变量 `APP_PROFILE` 覆盖。
    /// 缺省视为 `prod`：没有显式声明环境时按最严格的口径处理。
    /// 只有 `dev` 免登录（操作人固定为 `user_id = 0`），其它环境要求网关注入 `X-User-Id`。
    #[serde(default)]
    pub profile: Option<String>,
    /// jobx_scheduler
    #[serde(default = "JobxSchedulerConfig::default")]
    pub jobx_scheduler: JobxSchedulerConfig,
    /// jobx_task_reaper（僵尸任务巡检）
    #[serde(default = "JobxTaskReaperConfig::default")]
    pub jobx_task_reaper: JobxTaskReaperConfig,
    /// jobx_task_cleaner（过期任务清理）
    #[serde(default = "JobxTaskCleanerConfig::default")]
    pub jobx_task_cleaner: JobxTaskCleanerConfig,
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

impl AppConfig {
    /// 运行环境名；没有配置 profile 时按 `prod` 处理。
    pub fn profile_name(&self) -> &str {
        self.profile
            .as_deref()
            .map(str::trim)
            .filter(|profile| !profile.is_empty())
            .unwrap_or(DEFAULT_PROFILE)
    }

    /// 是否开发环境。
    pub fn is_dev(&self) -> bool {
        self.profile_name() == "dev"
    }
}

/// # 启动期校验
///
/// 鉴权由网关负责，jobx 只做「身份透传」，所以这里不校验令牌相关配置，
/// 只保证运行环境本身是受支持的值：拼错 profile 会让免登录判定走向不可预期的分支。
///
/// 必须在监听端口前调用，配置不合法时让进程直接起不来。
pub fn validate_app_config(config: &AppConfig) -> Result<(), String> {
    let profile = config.profile_name();
    if !ALLOWED_PROFILES.contains(&profile) {
        return Err(format!(
            "profile={profile} 不是受支持的运行环境（{}）：应通过 APP_PROFILE 指定",
            ALLOWED_PROFILES.join(" / ")
        ));
    }
    if config.is_dev() {
        tracing::warn!(
            "profile=dev：jobx 不做登录校验，所有请求的操作人固定为 user_id=0，禁止用于生产"
        );
    }
    Ok(())
}
