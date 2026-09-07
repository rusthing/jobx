use crate::api_client::JobApiClient;
use crate::api_client::ScheduleApiClient;
use crate::api_client::TaskApiClient;
use arc_swap::ArcSwapOption;
use config::Value;
use robotech::api_client::{ApiClientConfig, API_CLIENT_CONFIG_KEY};
use robotech::cfg::CfgError;
use robotech::micro_svc::FeignApiClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use wheel_rs::config_utils::has_config_changed;

static JOBX_API_CLIENT: ArcSwapOption<JobxApiClient> = ArcSwapOption::const_empty();
const JOBX_API_CLIENT_CONFIG_KEY: &str = "jobx";

pub struct JobxApiClient {
    pub job_client: JobApiClient,
    pub task_client: TaskApiClient,
    pub schedule_client: ScheduleApiClient,
}

pub fn get_jobx_api_client() -> Result<Arc<JobxApiClient>, CfgError> {
    JOBX_API_CLIENT.load_full().ok_or(CfgError::NotInit(
        "JOBX_API_CLIENT not initialized".to_string(),
    ))
}

pub async fn setup_jobx_api_client(
    apis_config: HashMap<String, ApiClientConfig>,
    changed: &Option<HashMap<String, Value>>,
) -> Result<(), CfgError> {
    info!("setup jobx api client...: {apis_config:?} {changed:?}");
    let key_prefix = format!("{}.{}", API_CLIENT_CONFIG_KEY, JOBX_API_CLIENT_CONFIG_KEY);
    if changed
        .as_ref()
        .map(|changed| has_config_changed(&key_prefix, changed))
        .unwrap_or(true)
    {
        let mut job_client: Option<JobApiClient> = None;
        let mut task_client: Option<TaskApiClient> = None;
        let mut schedule_client: Option<ScheduleApiClient> = None;
        for (key, api_client_config) in apis_config {
            if key == JOBX_API_CLIENT_CONFIG_KEY {
                info!("jobx api client config: {:?}", api_client_config);
                let feign_client = FeignApiClient::new(api_client_config.clone()).await;
                job_client = Some(JobApiClient::new(feign_client));
                let feign_client = FeignApiClient::new(api_client_config.clone()).await;
                task_client = Some(TaskApiClient::new(feign_client));
                let feign_client = FeignApiClient::new(api_client_config).await;
                schedule_client = Some(ScheduleApiClient::new(feign_client));
            }
        }
        let jobx_api_client = JobxApiClient {
            job_client: job_client.ok_or(CfgError::NotInit(
                "Job API client not initialized".to_string(),
            ))?,
            task_client: task_client.ok_or(CfgError::NotInit(
                "Task API client not initialized".to_string(),
            ))?,
            schedule_client: schedule_client.ok_or(CfgError::NotInit(
                "Schedule API client not initialized".to_string(),
            ))?,
        };
        JOBX_API_CLIENT.store(Some(Arc::new(jobx_api_client)));
    }
    Ok(())
}
