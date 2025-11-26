use crate::analyze::{Analyzer, StatusPayload};
use anyhow::Result;
use async_trait::async_trait;
use clap::Args;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[derive(Args, Debug)]
pub struct FaviconArgs {
    /// Output favicon to file
    #[arg(long)]
    favicon: Option<String>,
}

pub struct Favicon<'a> {
    args: &'a FaviconArgs,
}

async fn do_favicon_output(favicon: &str, output: &str) -> Result<()> {
    let data_url = data_url::DataUrl::process(favicon)?;
    let bytes = data_url.decode_to_vec()?;
    let mut file = File::create(output).await?;
    file.write_all(&bytes.0).await?;
    Ok(())
}

#[async_trait]
impl Analyzer for Favicon<'_> {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        self.args.favicon.is_some() && payload.favicon.is_some()
    }

    async fn analyze(&self, payload: &StatusPayload) {
        match do_favicon_output(
            &payload.favicon.as_ref().expect("No favicon provided"),
            &self
                .args
                .favicon
                .as_ref()
                .expect("No favicon output provided"),
        )
        .await
        {
            Ok(_) => (),
            Err(e) => log::error!("Favicon output error: {}", e),
        }
    }
}

impl Favicon<'_> {
    pub fn new(args: &'_ FaviconArgs) -> Favicon<'_> {
        Favicon { args }
    }
}
