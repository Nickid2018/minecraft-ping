mod favicon;
#[cfg(feature = "analyze-forge-info")]
mod forge_info;
mod game_mode;
mod motd;
mod ping;
mod player;
mod server_guid;
mod version;

use crate::analyze::favicon::FaviconArgs;
#[cfg(feature = "analyze-forge-info")]
use crate::analyze::forge_info::ForgeInfoArgs;
use crate::analyze::motd::{MotdArgs, sanitize_motd_args};
use crate::analyze::player::PlayerArgs;
use crate::mode::QueryMode;
use async_trait::async_trait;
use clap::{Args, ValueEnum, arg};
use serde_json::Value;

#[derive(Debug)]
pub struct PlayerInfo {
    pub id: String,
    pub uuid: String,
}

#[derive(Debug)]
pub enum MotdInfo {
    String(String),
    Component(Value),
}

#[derive(Debug)]
pub struct StatusPayload {
    pub mode: QueryMode,
    pub ping: i64,

    // players
    pub max_players: Option<i64>,
    pub player_count: Option<i64>,
    pub players: Option<Vec<PlayerInfo>>,

    // motd
    pub motd: Option<MotdInfo>,

    // version
    pub protocol: Option<i64>,
    pub version_name: Option<String>,

    // favicon
    pub favicon: Option<String>,

    // extra info
    pub full_extra: Option<Value>,
}

#[async_trait]
pub trait Analyzer {
    fn enabled(&self, payload: &StatusPayload) -> bool;
    async fn analyze(&self, payload: &StatusPayload);
}

pub struct AnalyzerTools<'a> {
    analyzers: Vec<Box<dyn Analyzer + 'a>>,
}

impl AnalyzerTools<'_> {
    pub async fn analyze(&self, payload: &StatusPayload) {
        for analyzer in self.analyzers.iter() {
            if analyzer.enabled(&payload) {
                analyzer.analyze(&payload).await;
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, ValueEnum)]
pub enum AvailableAnalyzers {
    #[value(name = "+")]
    AllDefaults,
    Ping,
    Version,
    ServerGuid,
    GameMode,
    Motd,
    Player,
    Favicon,
    #[cfg(feature = "analyze-forge-info")]
    ForgeInfo,
}

#[derive(Args, Debug)]
pub struct AnalyzerArgs {
    /// Set analyzers can be enabled, '+' for enabling all default analyzers
    #[arg(short='e', long, value_parser, value_delimiter = ',', default_values = ["ping", "version", "motd", "player", "favicon"])]
    analyzers: Vec<AvailableAnalyzers>,

    #[command(flatten)]
    motd_args: MotdArgs,
    #[command(flatten)]
    player_args: PlayerArgs,
    #[command(flatten)]
    favicon_args: FaviconArgs,
    #[cfg(feature = "analyze-forge-info")]
    #[command(flatten)]
    forge_info_args: ForgeInfoArgs,
}

pub fn sanitize_analyzer_args(args: &mut crate::BaseArgs) {
    let analyzers = &mut args.analyzer_args.analyzers;
    if analyzers.contains(&AvailableAnalyzers::AllDefaults) {
        analyzers.push(AvailableAnalyzers::Ping);
        analyzers.push(AvailableAnalyzers::Version);
        analyzers.push(AvailableAnalyzers::Motd);
        analyzers.push(AvailableAnalyzers::Player);
        analyzers.push(AvailableAnalyzers::Favicon);
    }
    if analyzers.contains(&AvailableAnalyzers::Motd) {
        sanitize_motd_args(args);
    }
}

pub fn init_analyzer_tools(args: &'_ AnalyzerArgs) -> AnalyzerTools<'_> {
    let mut analyzers: Vec<Box<dyn Analyzer>> = Vec::new();

    if args.analyzers.contains(&AvailableAnalyzers::Ping) {
        analyzers.push(Box::new(ping::Ping {}));
    }

    if args.analyzers.contains(&AvailableAnalyzers::Version) {
        analyzers.push(Box::new(version::Version {}));
    }

    if args.analyzers.contains(&AvailableAnalyzers::ServerGuid) {
        analyzers.push(Box::new(server_guid::ServerGuid {}));
    }

    if args.analyzers.contains(&AvailableAnalyzers::GameMode) {
        analyzers.push(Box::new(game_mode::GameMode {}));
    }

    if args.analyzers.contains(&AvailableAnalyzers::Motd) {
        analyzers.push(Box::new(motd::Motd::new(&args.motd_args)));
    }

    if args.analyzers.contains(&AvailableAnalyzers::Player) {
        analyzers.push(Box::new(player::Player::new(&args.player_args)));
    }

    if args.analyzers.contains(&AvailableAnalyzers::Favicon) {
        analyzers.push(Box::new(favicon::Favicon::new(&args.favicon_args)));
    }

    #[cfg(feature = "analyze-forge-info")]
    if args.analyzers.contains(&AvailableAnalyzers::ForgeInfo) {
        analyzers.push(Box::new(forge_info::ForgeInfo::new(&args.forge_info_args)));
    }

    AnalyzerTools { analyzers }
}
