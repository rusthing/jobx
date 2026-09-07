use crate::dto::*;
use crate::vo::{TaskExVo, TaskVo};
use robotech::macros::feign;
use robotech::micro_svc::FeignApiClient;

#[feign]
pub struct TaskApiClient {
    client: FeignApiClient,
}

impl TaskApiClient {
    pub fn new(client: FeignApiClient) -> Self {
        Self { client }
    }
}
