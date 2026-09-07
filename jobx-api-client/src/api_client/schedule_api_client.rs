use crate::dto::*;
use crate::vo::{ScheduleExVo, ScheduleVo};
use robotech::macros::feign;
use robotech::micro_svc::FeignApiClient;

#[feign]
pub struct ScheduleApiClient {
    client: FeignApiClient,
}

impl ScheduleApiClient {
    pub fn new(client: FeignApiClient) -> Self {
        Self { client }
    }
}
