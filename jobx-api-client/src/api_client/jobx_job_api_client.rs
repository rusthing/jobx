use crate::dto::*;
use crate::vo::{JobxJobExVo, JobxJobVo};
use robotech::macros::feign;
use robotech::micro_svc::FeignApiClient;

#[feign]
pub struct JobxJobApiClient {
    client: FeignApiClient,
}

impl JobxJobApiClient {
    pub fn new(client: FeignApiClient) -> Self {
        Self { client }
    }
}