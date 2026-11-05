use crate::api_client::jobx_group_api_client::JobxGroupApiClient;
use crate::api_client::jobx_job_api_client::JobxJobApiClient;
use crate::api_client::jobx_task_api_client::JobxTaskApiClient;
use robotech::macros::api_client;

#[api_client]
pub struct JobxApiClient {
    pub group: JobxGroupApiClient,
    pub job: JobxJobApiClient,
    pub task: JobxTaskApiClient,
}