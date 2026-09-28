use crate::api_client::jobx_job_api_client::JobxJobApiClient;
use crate::api_client::jobx_task_api_client::JobxTaskApiClient;
use robotech::macros::api_client;

#[api_client]
pub struct JobxApiClient {
    pub job_client: JobxJobApiClient,
    pub task_client: JobxTaskApiClient,
}