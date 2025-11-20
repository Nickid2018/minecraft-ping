mod favicon;
mod motd;
mod ping;
mod player;
mod version;

use crate::analyze::favicon::FaviconArgs;
use crate::analyze::player::PlayerArgs;
use async_trait::async_trait;
use clap::{Args, ValueEnum};
use serde_json::Value;

#[derive(Debug)]
pub struct PlayerInfo {
    pub id: String,
    pub uuid: String,
}

#[derive(Debug)]
pub enum MOTD {
    String(String),
    Component(Value),
}

#[derive(Debug)]
pub struct StatusPayload {
    pub ping: i64,

    // players
    pub max_players: Option<i64>,
    pub player_count: Option<i64>,
    pub players: Option<Vec<PlayerInfo>>,

    // motd
    pub motd: Option<MOTD>,

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

pub struct AnalyzerTools {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalyzerTools {
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
    PING,
    VERSION,
    MOTD,
    PLAYER,
    FAVICON,
}

#[derive(Args, Debug)]
pub struct AnalyzerArgs {
    /// Set analyzers can be enabled
    #[arg(short='e', long, value_parser, value_delimiter = ',', default_values = ["ping", "version", "motd", "player", "favicon"])]
    analyzers: Vec<AvailableAnalyzers>,

    #[command(flatten)]
    player_args: PlayerArgs,
    #[command(flatten)]
    favicon_args: FaviconArgs,
}

pub fn init_analyzer_tools(args: &AnalyzerArgs) -> AnalyzerTools {
    let mut analyzers: Vec<Box<dyn Analyzer>> = Vec::new();

    if args.analyzers.contains(&AvailableAnalyzers::PING) {
        analyzers.push(Box::new(ping::Ping {}));
    }

    if args.analyzers.contains(&AvailableAnalyzers::VERSION) {
        analyzers.push(Box::new(version::Version {}));
    }

    if args.analyzers.contains(&AvailableAnalyzers::MOTD) {
        analyzers.push(Box::new(motd::Motd {}));
    }

    if args.analyzers.contains(&AvailableAnalyzers::PLAYER) {
        analyzers.push(Box::new(player::Player::new(&args.player_args)));
    }

    if args.analyzers.contains(&AvailableAnalyzers::FAVICON) {
        analyzers.push(Box::new(favicon::Favicon::new(&args.favicon_args)));
    }

    AnalyzerTools { analyzers }
}
