use crate::analyze::{Analyzer, StatusPayload};
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

pub struct Favicon {
    favicon: Option<String>,
}

async fn do_favicon_output(favicon: &str, output: &str) -> std::io::Result<()> {
    let data_url = data_url::DataUrl::process(favicon).map_err(crate::util::wrap_other)?;
    let bytes = data_url.decode_to_vec().map_err(crate::util::wrap_other)?;
    let mut file = File::create(output).await?;
    file.write_all(&bytes.0).await?;
    Ok(())
}

#[async_trait]
impl Analyzer for Favicon {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        self.favicon.is_some() && payload.favicon.is_some()
    }

    async fn analyze(&self, payload: &StatusPayload) {
        match do_favicon_output(
            &payload.favicon.as_ref().unwrap(),
            &self.favicon.as_ref().unwrap(),
        )
        .await
        {
            Ok(_) => (),
            Err(e) => log::error!("Favicon output error: {}", e),
        }
    }
}

impl Favicon {
    pub fn new(args: &FaviconArgs) -> Favicon {
        Favicon {
            favicon: args.favicon.clone(),
        }
    }
}
