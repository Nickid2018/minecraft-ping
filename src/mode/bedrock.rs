use crate::analyze::{MotdInfo, StatusPayload};
use crate::mode::QueryModeHandler;
use crate::network::resolve::resolve_addr;
use crate::network::util;
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use serde_json::json;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use crate::mode::QueryMode::BEDROCK;

const MAGIC_HIGH: u64 = 0x00ffff00fefefefeu64;
const MAGIC_LOW: u64 = 0xfdfdfdfd12345678u64;

async fn single_ip_check(addr: &SocketAddr) -> std::io::Result<StatusPayload> {
    let timeout_time = Duration::from_secs(5);
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(addr).await?;
    log::trace!("Connected to {}", addr);

    let timestamp = util::now_timestamp();
    let mut packet = Vec::from([1u8]);
    packet.put_i64(timestamp);
    packet.put_u64(MAGIC_HIGH);
    packet.put_u64(MAGIC_LOW);
    packet.put_u16(0);
    socket.send_to(&packet, addr).await?;
    log::trace!("Sent Unconnected Ping packet");

    let mut recv_buf = [0u8; 1024];
    let recv = timeout(timeout_time, socket.recv_from(&mut recv_buf)).await??;
    log::trace!("Received response");

    let mut bytes = BytesMut::from(&recv_buf[..recv.0]);
    if bytes.get_u8() != 0x1C {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Unexpected response byte",
        ));
    }

    let server_clock = bytes.get_i64();
    let server_guid = bytes.get_u64();
    let magic_high = bytes.get_u64();
    let magic_low = bytes.get_u64();
    let str_len = bytes.get_u16();

    if magic_high != MAGIC_HIGH || magic_low != MAGIC_LOW {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Invalid magic number",
        ));
    }
    if str_len as usize != recv.0 - 35 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Invalid string length",
        ));
    }

    let ping = util::now_timestamp() - server_clock;
    let resp = String::from_utf8(bytes.to_vec()).map_err(crate::util::wrap_other)?;
    log::trace!("Ping response: {}", resp);

    let parts = resp.split(';').collect::<Vec<_>>();
    if parts.len() < 9 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Invalid response",
        ));
    }

    Ok(StatusPayload {
        mode: BEDROCK,
        ping,
        max_players: Some(util::parse_i64(parts[5])?),
        player_count: Some(util::parse_i64(parts[4])?),
        players: None,
        motd: Some(MotdInfo::String(format!("{}\n{}", parts[1], parts[7]))),
        protocol: Some(util::parse_i64(parts[2])?),
        version_name: Some(parts[3].to_string()),
        favicon: None,
        full_extra: Some(json!({"server_guid": server_guid, "game_mode": parts[8].to_string()})),
    })
}

async fn check_bedrock_server(addr_vec: &Vec<SocketAddr>) -> std::io::Result<StatusPayload> {
    for addr in addr_vec {
        match single_ip_check(&addr).await {
            Ok(r) => {
                return Ok(r);
            }
            Err(e) => {
                log::warn!("Failed to check available server ip {}: {}", addr, e);
                continue;
            }
        }
    }
    Err(std::io::Error::new(ErrorKind::NotFound, "No server found"))
}

pub struct BedrockQuery;

#[async_trait]
impl QueryModeHandler for BedrockQuery {
    async fn do_query(&self, addr: &str) -> std::io::Result<StatusPayload> {
        let mut res = resolve_addr(addr, 19132);
        let addrs = res.get_or_insert_default();
        check_bedrock_server(addrs).await
    }
}

impl BedrockQuery {
    pub fn new() -> BedrockQuery {
        BedrockQuery {}
    }
}
