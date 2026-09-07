use crate::dto::*;
use crate::vo::{JobExVo, JobVo};
use robotech::macros::feign;
use robotech::micro_svc::FeignApiClient;

#[feign]
pub struct JobApiClient {
    client: FeignApiClient,
}

impl JobApiClient {
    pub fn new(client: FeignApiClient) -> Self {
        Self { client }
    }
}
