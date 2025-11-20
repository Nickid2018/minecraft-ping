use crate::analyze::{Analyzer, StatusPayload};
use async_trait::async_trait;

pub struct Version;

#[async_trait]
impl Analyzer for Version {
    fn enabled(&self, _payload: &StatusPayload) -> bool {
        true
    }

    async fn analyze(&self, payload: &StatusPayload) {
        if let Some(extra) = payload.full_extra.as_ref()
            && let Some(legacy) = extra["legacy_version"].as_i64()
        {
            log::info!("Legacy Response Version: {}", legacy);
        }

        let protocol_num = if let Some(protocol) = payload.protocol {
            protocol.to_string()
        } else {
            "<UNKNOWN>".to_string()
        };
        let version_name = if let Some(name) = payload.version_name.as_ref() {
            name
        } else {
            "<UNKNOWN>"
        };
        log::info!("Version: {} ({})", version_name, protocol_num);
    }
}
