use crate::analyze::*;
use crate::mode::QueryMode::LEGACY;
use crate::mode::QueryModeHandler;
use crate::mode::java::JavaModeArgs;
use crate::network::connection::connect_tcp;
use crate::network::resolve::{resolve_server_srv, sanitize_addr};
use crate::network::util;
use crate::network::util::{io_timeout, now_timestamp};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use bytes::BufMut;
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinSet;

const LEGACY_HEADER: [u8; 27] = [
    0xFE, 0x01, 0xFA, 0x00, 0x0B, 0x00, 0x4D, 0x00, 0x43, 0x00, 0x7C, 0x00, 0x50, 0x00, 0x69, 0x00,
    0x6E, 0x00, 0x67, 0x00, 0x48, 0x00, 0x6F, 0x00, 0x73, 0x00, 0x74,
];

async fn single_ip_check(addr: &str, port: u16, stream: &mut TcpStream) -> Result<StatusPayload> {
    let time = Duration::from_secs(5);

    let utf16 = addr.encode_utf16().collect::<Vec<_>>();
    let mut buf = Vec::from(LEGACY_HEADER);
    let packet_len = (utf16.len() * 2 + 7) as u16;
    buf.put_u16(packet_len);
    buf.push(73);
    buf.put_u16(utf16.len() as u16);
    for short in utf16 {
        buf.put_u16(short);
    }
    buf.put_u16(0);
    buf.put_u16(port);

    let send_time = now_timestamp();
    stream.write_all(&buf).await?;
    log::trace!("Legacy query sent, packet length = {}", packet_len);

    let mut recv_buffer = [0u8; 3];
    io_timeout(time, stream.read_exact(&mut recv_buffer), "Handshake").await?;
    let ping = now_timestamp() - send_time;
    log::trace!("Legacy query received, ping = {}", ping);
    if recv_buffer[0] != 0xFF {
        return Err(anyhow!("Legacy header should be 0xFF"));
    }

    let recv_len = u16::from_be_bytes([recv_buffer[1], recv_buffer[2]]) * 2;
    let mut recv = vec![0; recv_len as usize];
    stream.read_exact(&mut recv).await?;
    let u16buf = recv
        .chunks_exact(2)
        .into_iter()
        .map(|a| u16::from_be_bytes([a[0], a[1]]))
        .collect::<Vec<_>>();
    let str = String::from_utf16(&u16buf)?;
    log::trace!("Legacy query received from {}: {}", addr, str);

    if str.starts_with("\u{00A7}1\0") {
        log::debug!("Legacy query version 1");
        let parts = str.split('\0').collect::<Vec<_>>();
        if parts.len() < 6 {
            return Err(anyhow!("Legacy query string is invalid"));
        }
        if parts.len() > 6 {
            log::warn!("Legacy query string has too many parts");
            log::warn!("Data will be collected, but can not ensure data is correct");
        }
        Ok(StatusPayload {
            mode: LEGACY,
            ping,
            max_players: Some(util::parse_i64(parts[5])?),
            player_count: Some(util::parse_i64(parts[4])?),
            players: None,
            motd: Some(MotdInfo::String(parts[3].to_string())),
            protocol: Some(util::parse_i64(parts[1])?),
            version_name: Some(parts[2].to_string()),
            favicon: None,
            full_extra: Some(json!({"legacy_version": 1})),
        })
    } else {
        log::debug!("Legacy query version 0");
        let parts = str.split('\u{00A7}').collect::<Vec<_>>();
        if parts.len() < 3 {
            return Err(anyhow!("Legacy query string is invalid"));
        }
        if parts.len() > 3 {
            log::warn!("Legacy query string has too many parts");
            log::warn!("Data will be collected, but can not ensure data is correct");
        }
        Ok(StatusPayload {
            mode: LEGACY,
            ping,
            max_players: Some(util::parse_i64(parts[2])?),
            player_count: Some(util::parse_i64(parts[1])?),
            players: None,
            motd: Some(MotdInfo::String(parts[0].to_string())),
            protocol: None,
            version_name: None,
            favicon: None,
            full_extra: Some(json!({"legacy_version": 0})),
        })
    }
}

async fn safe_ip_check(addr: String, port: u16, mut stream: TcpStream) -> Result<StatusPayload> {
    match single_ip_check(&addr, port, &mut stream).await {
        Ok(resp) => Ok(resp),
        Err(e) => Err(anyhow!("Protocol error in <{}:{}>: {}", addr, port, e)),
    }
}

async fn check_legacy_server(
    addr: &str,
    port: u16,
    streams: Vec<TcpStream>,
) -> Result<StatusPayload> {
    let mut set = JoinSet::new();

    for stream in streams {
        set.spawn(safe_ip_check(addr.to_string(), port, stream));
    }

    while let Some(join_res) = set.join_next().await {
        if let Ok(res) = join_res {
            match res {
                Ok(res) => return Ok(res),
                Err(e) => log::warn!("{}", e),
            }
        }
    }

    Err(anyhow!("No server found"))
}

pub struct LegacyQuery<'a> {
    args: &'a JavaModeArgs,
}

#[async_trait]
impl QueryModeHandler for LegacyQuery<'_> {
    async fn do_query(&self, addr: &str) -> Result<StatusPayload> {
        let mut addrs = vec![addr.to_string()];
        if !self.args.no_srv {
            addrs.splice(0..0, resolve_server_srv(addr).await);
        }
        for addr in addrs {
            let (host, port) = sanitize_addr(&addr, 25565)?;
            match connect_tcp(&host, port, Duration::new(5, 0)).await {
                Ok(streams) => {
                    if streams.is_empty() {
                        log::warn!("No successful connection found for <{}:{}>", addr, port);
                        continue;
                    }
                    match check_legacy_server(&host, port, streams).await {
                        Ok(status) => return Ok(status),
                        Err(e) => log::warn!("Failed to check <{}:{}>: {}", addr, port, e),
                    }
                }
                Err(e) => {
                    log::warn!("Failed to connect to <{}:{}>: {}", addr, port, e);
                }
            }
        }
        Err(anyhow!("No server found"))
    }
}

impl LegacyQuery<'_> {
    pub fn new(args: &'_ JavaModeArgs) -> LegacyQuery<'_> {
        LegacyQuery { args }
    }
}
