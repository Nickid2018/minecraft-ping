use crate::analyze::{Analyzer, StatusPayload};
use crate::mode::QueryMode::BEDROCK;
use async_trait::async_trait;

pub struct GameMode;

#[async_trait]
impl Analyzer for GameMode {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        payload.mode == BEDROCK
    }

    async fn analyze(&self, payload: &StatusPayload) {
        if let Some(mode) = payload
            .full_extra
            .as_ref()
            .map(|x| x["game_mode"].as_str())
            .flatten()
        {
            log::info!("Game Mode: {}", mode);
        }
    }
}
