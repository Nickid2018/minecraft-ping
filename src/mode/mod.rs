use crate::analyze::StatusPayload;
use crate::mode::QueryMode::*;
use crate::mode::bedrock::BedrockQuery;
use crate::mode::java::{JavaModeArgs, JavaQuery};
#[cfg(feature = "ping-legacy")]
use crate::mode::legacy::LegacyQuery;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use clap::{Args, ValueEnum};
use std::collections::HashMap;

pub mod bedrock;
pub mod java;
#[cfg(feature = "ping-legacy")]
pub mod legacy;

#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq, ValueEnum)]
pub enum QueryMode {
    JAVA,
    BEDROCK,
    #[cfg(feature = "ping-legacy")]
    LEGACY,
}

#[async_trait]
trait QueryModeHandler {
    async fn do_query(&self, addr: &str) -> Result<StatusPayload>;
}

pub struct QueryEngine<'a> {
    modes: HashMap<QueryMode, Box<dyn QueryModeHandler + 'a>>,
}

impl QueryEngine<'_> {
    pub async fn query(&self, mode: QueryMode, addr: &str) -> Result<StatusPayload> {
        if let Some(handler) = self.modes.get(&mode) {
            handler.do_query(addr).await
        } else {
            Err(anyhow!("No available query mode"))
        }
    }
}

#[derive(Args, Debug)]
pub struct ModeArgs {
    #[command(flatten)]
    java: JavaModeArgs,
}

pub fn init_query_engine(args: &'_ ModeArgs) -> QueryEngine<'_> {
    let mut modes: HashMap<QueryMode, Box<dyn QueryModeHandler>> = HashMap::new();
    modes.insert(JAVA, Box::new(JavaQuery::new(&args.java)));
    modes.insert(BEDROCK, Box::new(BedrockQuery::new()));
    #[cfg(feature = "ping-legacy")]
    modes.insert(LEGACY, Box::new(LegacyQuery::new(&args.java)));
    QueryEngine { modes }
}
