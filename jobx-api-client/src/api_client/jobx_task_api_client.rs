use crate::dto::*;
use crate::vo::{JobxTaskExVo, JobxTaskVo};
use robotech::macros::feign;
use robotech::micro_svc::FeignApiClient;

#[feign]
pub struct JobxTaskApiClient {
    client: FeignApiClient,
}

impl JobxTaskApiClient {
    pub fn new(client: FeignApiClient) -> Self {
        Self { client }
    }
}