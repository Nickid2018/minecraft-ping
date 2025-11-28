use crate::analyze::{Analyzer, StatusPayload};
use crate::network::schema::{read_string, read_var_int_buf};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use clap::Args;

#[derive(Args, Debug)]
pub struct ForgeInfoArgs {
    /// Display forge channels
    #[arg(long)]
    display_channels: bool,
}

pub struct ForgeInfo<'a> {
    args: &'a ForgeInfoArgs,
}

async fn try_analyze_encoded(data: &str, display_channels: bool) -> Result<()> {
    let chars = data.encode_utf16().collect::<Vec<u16>>();
    if chars.len() < 2 {
        return Err(anyhow!("ForgeData too short"));
    }

    let buffer_len = chars[0] as u32 | (chars[1] as u32) << 15;
    let mut buffer = BytesMut::with_capacity(buffer_len as usize);
    let mut bits_in_buf = 0;
    let mut buf = 0;
    for char in chars[2..].iter() {
        while bits_in_buf >= 8 {
            buffer.put_u8((buf & 0xFF) as u8);
            buf >>= 8;
            bits_in_buf -= 8;
        }
        buf |= ((*char as u32) & 32767) << bits_in_buf;
        bits_in_buf += 15;
    }
    while bits_in_buf > 0 {
        buffer.put_u8((buf & 0xFF) as u8);
        buf >>= 8;
        bits_in_buf -= 8;
    }
    log::trace!("ForgeData: {:?}", String::from_utf8(buffer.to_vec()));

    if buffer.try_get_u8()? != 0 {
        log::info!("Server truncated mod information");
    }

    let size = buffer.try_get_u16()?;
    for _ in 0..size {
        let flag = read_var_int_buf(&mut buffer)?;
        let ch_size = flag >> 1 & (!(1 << 31));
        let ignore_server_only = flag & 1 != 0;
        let name = read_string(&mut buffer)?;
        let version = if ignore_server_only {
            "<UNCHECKED>".to_string()
        } else {
            read_string(&mut buffer)?
        };
        if version == "" {
            log::info!("Mod: {}", name);
        } else {
            log::info!("Mod: {} ({})", name, version);
        }

        for _ in 0..ch_size {
            let path = read_string(&mut buffer)?;
            let ver = read_var_int_buf(&mut buffer)?;
            let required = buffer.try_get_u8()? != 0;
            if !display_channels {
                continue;
            }
            if required {
                log::info!("  Channel* {} ({})", path, ver);
            } else {
                log::info!("  Channel  {} ({})", path, ver);
            }
        }
    }

    if !display_channels {
        return Ok(());
    }

    let non_mod_channels = read_var_int_buf(&mut buffer)?;
    if non_mod_channels == 0 {
        return Ok(());
    }
    log::info!("Non-mod channels:");
    for _ in 0..non_mod_channels {
        let path = read_string(&mut buffer)?;
        let ver = read_var_int_buf(&mut buffer)?;
        if buffer.try_get_u8()? != 0 {
            log::info!("  Channel* {} ({})", path, ver);
        } else {
            log::info!("  Channel  {} ({})", path, ver);
        }
    }

    Ok(())
}

#[async_trait]
impl Analyzer for ForgeInfo<'_> {
    fn enabled(&self, payload: &StatusPayload) -> bool {
        payload
            .full_extra
            .as_ref()
            .map(|m| m["forgeData"].as_object())
            .flatten()
            .is_some()
    }

    async fn analyze(&self, payload: &StatusPayload) {
        let forge_data = &payload.full_extra.as_ref().expect("Forge data")["forgeData"];
        log::info!(
            "Forge Mod Loader (Network Version {})",
            forge_data["fmlNetworkVersion"]
                .as_i64()
                .map(|i| i.to_string())
                .unwrap_or("<unknown version>".to_string())
        );
        if let Some(data) = forge_data["d"].as_str() {
            try_analyze_encoded(data, self.args.display_channels)
                .await
                .err()
                .map(|e| log::error!("{}", e));
        } else {
            if forge_data["truncated"].as_bool().unwrap_or(false) {
                log::info!("Server truncated mod information");
            }

            let default_vec = vec![];
            let mod_list = forge_data["mods"].as_array().unwrap_or(&default_vec);
            let ch_list = forge_data["channels"].as_array().unwrap_or(&default_vec);

            for mod_data in mod_list {
                let name = mod_data["modId"].as_str().unwrap_or("<unknown name>");
                let version = mod_data["modmarker"].as_str().unwrap_or("");
                if version == "" {
                    log::info!("Mod: {}", name);
                } else {
                    log::info!("Mod: {} ({})", name, version);
                }
            }

            if self.args.display_channels {
                for ch in ch_list {
                    let path = ch["res"].as_str().unwrap_or("<unknown path>");
                    let ver = ch["version"].as_i64().unwrap_or(0);
                    if ch["required"].as_bool().unwrap_or(false) {
                        log::info!("  Channel* {} ({})", path, ver);
                    } else {
                        log::info!("  Channel  {} ({})", path, ver);
                    }
                }
            }
        }
    }
}

impl ForgeInfo<'_> {
    pub fn new(args: &'_ ForgeInfoArgs) -> ForgeInfo<'_> {
        ForgeInfo { args }
    }
}
