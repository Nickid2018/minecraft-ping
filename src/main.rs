mod analyze;
mod logger;
mod mode;
mod network;
mod util;

use crate::analyze::{init_analyzer_tools, AnalyzerArgs};
use crate::mode::{init_query_engine, ModeArgs};
use clap::Parser;
use logger::LogLevel;
use mode::QueryMode;
use std::process::ExitCode;

/// Program for minecraft server pinging.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct BaseArgs {
    /// Address to query
    #[arg()]
    address: String,
    /// Query mode
    #[arg(short, long, value_parser, value_delimiter = ',', num_args = 1.., default_values = ["java", "bedrock"])]
    mode: Vec<QueryMode>,
    /// Use all modes in `mode` option instead of returning when one mode succeed
    #[arg(long)]
    run_all_modes: bool,

    #[command(flatten)]
    mode_args: ModeArgs,
    #[command(flatten)]
    analyzer_args: AnalyzerArgs,

    /// Log level for output
    #[arg(short, long, default_value = "info")]
    log_level: LogLevel,
    /// Disable colorize output
    #[arg(long)]
    no_color: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = BaseArgs::parse();
    logger::init(args.log_level, args.no_color).unwrap();

    let engine = init_query_engine(&args.mode_args);
    let analyzers = init_analyzer_tools(&args.analyzer_args);

    let mut fail_count = 0;
    for mode in args.mode {
        match engine.query(mode, &args.address).await {
            Ok(payload) => {
                log::info!("Query successful use mode {:?}", mode);
                analyzers.analyze(&payload).await;
                if !args.run_all_modes {
                    break;
                }
            }
            Err(e) => {
                fail_count += 1;
                log::error!("Failed for mode {:?}: {}", mode, e);
            }
        }
    }

    ExitCode::from(fail_count)
}
