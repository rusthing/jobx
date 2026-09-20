use crate::dto::*;
use crate::vo::{JobxTaskExVo, JobxTaskVo};
use robotech::macros::feign;
use robotech::micro_svc::FeignApiClient;
use robotech::api::Ro;

#[feign]
pub struct JobxTaskApiClient {
    client: FeignApiClient,
}

impl JobxTaskApiClient {
    pub fn new(client: FeignApiClient) -> Self {
        Self { client }
    }

    pub async fn take(
        &self,
        dto: &JobxTaskAddDto,
    ) -> Result<Ro<JobxTaskVo>, robotech::api_client::ApiClientError> {
        let url = "/jobx/task/take".to_string();
        let headers = Self::build_headers(dto._current_user_id.value(), dto._current_ms.map(|m| m.value()))?;
        self.client
            .request::<JobxTaskAddDto, JobxTaskVo>(
                ::reqwest::Method::POST,
                &url,
                None,
                Some(dto),
                Some(&headers),
            )
            .await
    }
}