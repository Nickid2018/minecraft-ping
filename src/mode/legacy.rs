use crate::analyze::*;
use crate::mode::QueryMode::LEGACY;
use crate::mode::QueryModeHandler;
use crate::mode::java::{JavaModeArgs, add_srv};
use crate::network::resolve::resolve_addr;
use crate::network::util;
use crate::network::util::{io_timeout, now_timestamp};
use crate::util::make_tcp_socket;
use async_trait::async_trait;
use bytes::BufMut;
use serde_json::json;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;

const LEGACY_HEADER: [u8; 27] = [
    0xFE, 0x01, 0xFA, 0x00, 0x0B, 0x00, 0x4D, 0x00, 0x43, 0x00, 0x7C, 0x00, 0x50, 0x00, 0x69, 0x00,
    0x6E, 0x00, 0x67, 0x00, 0x48, 0x00, 0x6F, 0x00, 0x73, 0x00, 0x74,
];

async fn single_ip_check(addr: &SocketAddr) -> std::io::Result<StatusPayload> {
    let time = Duration::from_secs(5);
    let socket = make_tcp_socket(addr)?;
    let mut stream = io_timeout(time, socket.connect(*addr), "Connection").await??;

    let ip_str = addr.ip().to_string();
    let utf16 = ip_str.encode_utf16().collect::<Vec<_>>();
    let mut buf = Vec::from(LEGACY_HEADER);
    let packet_len = (utf16.len() * 2 + 7) as u16;
    buf.put_u16(packet_len);
    buf.push(73);
    buf.put_u16(utf16.len() as u16);
    for short in utf16 {
        buf.put_u16(short);
    }
    buf.put_u16(0);
    buf.put_u16(addr.port());

    let send_time = now_timestamp();
    stream.write_all(&buf).await?;
    log::trace!("Legacy query sent, packet length = {}", packet_len);

    let mut recv_buffer = [0u8; 3];
    io_timeout(time, stream.read_exact(&mut recv_buffer), "Handshake").await??;
    let ping = now_timestamp() - send_time;
    log::trace!("Legacy query received, ping = {}", ping);
    if recv_buffer[0] != 0xFF {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Legacy header should be 0xFF",
        ));
    }

    let recv_len = u16::from_be_bytes([recv_buffer[1], recv_buffer[2]]) * 2;
    let mut recv = vec![0; recv_len as usize];
    stream.read_exact(&mut recv).await?;
    let u16buf = recv
        .chunks_exact(2)
        .into_iter()
        .map(|a| u16::from_be_bytes([a[0], a[1]]))
        .collect::<Vec<_>>();
    let str = String::from_utf16(&u16buf).map_err(crate::util::wrap_invalid)?;
    log::trace!("Legacy query received from {}: {}", addr, str);

    if str.starts_with("\u{00A7}1\0") {
        log::debug!("Legacy query version 1");
        let parts = str.split('\0').collect::<Vec<_>>();
        if parts.len() < 6 {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "Legacy query string is invalid",
            ));
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
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "Legacy query string is invalid",
            ));
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

async fn safe_ip_check(addr: SocketAddr) -> std::io::Result<StatusPayload> {
    match single_ip_check(&addr).await {
        Ok(r) => Ok(r),
        Err(e) => {
            log::warn!("Failed to check available server ip {}: {}", addr, e);
            Err(e)
        }
    }
}

async fn check_legacy_server(addr_vec: Vec<SocketAddr>) -> std::io::Result<StatusPayload> {
    let mut set = JoinSet::new();

    for addr in addr_vec {
        set.spawn(safe_ip_check(addr));
    }

    while let Some(join_res) = set.join_next().await {
        if let Ok(res) = join_res
            && res.is_ok()
        {
            return res;
        }
    }

    Err(std::io::Error::new(ErrorKind::NotFound, "No server found"))
}

pub struct LegacyQuery<'a> {
    args: &'a JavaModeArgs,
}

#[async_trait]
impl QueryModeHandler for LegacyQuery<'_> {
    async fn do_query(&self, addr: &str) -> std::io::Result<StatusPayload> {
        let res = resolve_addr(addr, 25565);
        let mut addrs = res.unwrap_or(vec![]);

        if !self.args.no_srv {
            add_srv(addr, &mut addrs).await;
        }

        check_legacy_server(addrs).await
    }
}

impl LegacyQuery<'_> {
    pub fn new(args: &'_ JavaModeArgs) -> LegacyQuery<'_> {
        LegacyQuery { args }
    }
}
