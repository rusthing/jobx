use crate::api_client::jobx_job_api_client::JobxJobApiClient;
use crate::api_client::jobx_task_api_client::JobxTaskApiClient;
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
const JOBX_API_CLIENT_CONFIG_KEY: &str = "utils";

pub struct JobxApiClient {
    pub job_client: JobxJobApiClient,
    pub task_client: JobxTaskApiClient,
}

pub fn get_jobx_api_client() -> Result<Arc<JobxApiClient>, CfgError> {
    JOBX_API_CLIENT
        .load_full()
        .ok_or(CfgError::NotInit(
            "JOBX_API_CLIENT not initialized".to_string(),
        ))
}

pub async fn setup_jobx_api_client(
    apis_config: HashMap<String, ApiClientConfig>,
    changed: &Option<HashMap<String, Value>>,
) -> Result<(), CfgError> {
    info!("setup utils api client...: {apis_config:?} {changed:?}");
    let key_prefix = format!("{}.{}", API_CLIENT_CONFIG_KEY, JOBX_API_CLIENT_CONFIG_KEY);
    if changed
        .as_ref()
        .map(|changed| has_config_changed(&key_prefix, changed))
        .unwrap_or(true)
    {
        let mut job_client: Option<JobxJobApiClient> = None;
        let mut task_client: Option<JobxTaskApiClient> = None;
        for (key, api_client_config) in apis_config {
            if key == JOBX_API_CLIENT_CONFIG_KEY {
                info!("utils api client config: {:?}", api_client_config);
                job_client = Some(JobxJobApiClient::new(
                    FeignApiClient::new(api_client_config.clone()).await,
                ));
                task_client = Some(JobxTaskApiClient::new(
                    FeignApiClient::new(api_client_config).await,
                ));
            }
        }
        let job_client = job_client.ok_or(CfgError::NotInit(
            "JOBX API client not initialized".to_string(),
        ))?;
        let task_client = task_client.ok_or(CfgError::NotInit(
            "JOBX API client not initialized".to_string(),
        ))?;

        JOBX_API_CLIENT.store(Some(Arc::new(JobxApiClient {
            job_client,
            task_client,
        })));
    }
    Ok(())
}