use async_trait::async_trait;
use crate::analyze::{Analyzer, StatusPayload, MOTD};

pub struct Motd;

#[async_trait]
impl Analyzer for Motd {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        payload.motd.is_some()
    }

    async fn analyze(&self, payload: &StatusPayload) {
        let motd = payload.motd.as_ref().unwrap();
        match motd {
            MOTD::String(s) => {
                log::info!("{}", s);
            }
            MOTD::Component(s) => {
                log::info!("{}", s.to_string());
            }
        }
    }
}