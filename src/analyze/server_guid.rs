use crate::analyze::{Analyzer, StatusPayload};
use crate::mode::QueryMode::BEDROCK;
use async_trait::async_trait;

pub struct ServerGuid;

#[async_trait]
impl Analyzer for ServerGuid {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        payload.mode == BEDROCK
    }

    async fn analyze(&self, payload: &StatusPayload) {
        if let Some(guid) = payload
            .full_extra
            .as_ref()
            .map(|x| x["server_guid"].as_u64())
            .flatten()
        {
            log::info!("Server guid: {}", guid);
        }
    }
}
