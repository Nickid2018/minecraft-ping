use crate::analyze::{Analyzer, StatusPayload};
use async_trait::async_trait;
use clap::Args;

#[derive(Args, Debug)]
pub struct PlayerArgs {
    /// Do not output player lists
    #[arg(long)]
    no_player_list: bool,
    /// Do not output player UUIDs
    #[arg(long)]
    no_uuid: bool,
    /// Hide anonymous players
    #[arg(long)]
    hide_anonymous: bool,
}

pub struct Player<'a> {
    args: &'a PlayerArgs,
}

#[async_trait]
impl Analyzer for Player<'_> {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        payload.max_players.is_some() && payload.player_count.is_some()
    }

    async fn analyze(&self, payload: &StatusPayload) {
        log::info!(
            "Players: {}/{}",
            payload.player_count.expect("player count should exist"),
            payload.max_players.expect("max player count should exist")
        );
        if !self.args.no_player_list
            && let Some(players) = payload.players.as_ref()
        {
            for player in players {
                if self.args.hide_anonymous && player.uuid == "00000000-0000-0000-0000-000000000000"
                {
                    continue;
                }
                if self.args.no_uuid {
                    log::info!("  {}", player.id);
                } else {
                    log::info!("  {:20} ({})", player.id, player.uuid);
                }
            }
        }
    }
}

impl Player<'_> {
    pub fn new(args: &'_ PlayerArgs) -> Player<'_> {
        Player { args }
    }
}
