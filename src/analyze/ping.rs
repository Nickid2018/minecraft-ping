use async_trait::async_trait;
use crate::analyze::{Analyzer, StatusPayload};

pub struct Ping;

#[async_trait]
impl Analyzer for Ping {
    fn enabled(&self, _payload: &StatusPayload) -> bool {
        true
    }

    async fn analyze(&self, payload: &StatusPayload) {
        log::info!("Ping to server: {}ms", payload.ping);
    }
}