use crate::dto::*;
use crate::vo::{JobxGroupExVo, JobxGroupVo};
use robotech::macros::feign;
use robotech::micro_svc::FeignApiClient;

#[feign]
pub struct JobxGroupApiClient {
    client: FeignApiClient,
}

impl JobxGroupApiClient {
    pub fn new(client: FeignApiClient) -> Self {
        Self { client }
    }
}
