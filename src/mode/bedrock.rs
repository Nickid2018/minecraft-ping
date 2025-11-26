use crate::analyze::{MotdInfo, StatusPayload};
use crate::mode::QueryMode::BEDROCK;
use crate::mode::QueryModeHandler;
use crate::network::resolve::{resolve_addr, sanitize_addr};
use crate::network::util;
use crate::network::util::io_timeout;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use serde_json::json;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::task::JoinSet;

const MAGIC_HIGH: u64 = 0x00ffff00fefefefeu64;
const MAGIC_LOW: u64 = 0xfdfdfdfd12345678u64;

async fn single_ip_check(addr: SocketAddr) -> Result<StatusPayload> {
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
    let recv = io_timeout(timeout_time, socket.recv_from(&mut recv_buf), "Recv").await?;
    log::trace!("Received response from {}", addr);

    let mut bytes = BytesMut::from(&recv_buf[..recv.0]);
    if bytes.get_u8() != 0x1C {
        return Err(anyhow!("Unexpected response byte"));
    }

    let server_clock = bytes.get_i64();
    let server_guid = bytes.get_u64();
    let magic_high = bytes.get_u64();
    let magic_low = bytes.get_u64();
    let str_len = bytes.get_u16();

    if magic_high != MAGIC_HIGH || magic_low != MAGIC_LOW {
        return Err(anyhow!("Invalid magic number"));
    }
    if str_len as usize != recv.0 - 35 {
        return Err(anyhow!("Invalid string length",));
    }

    let ping = util::now_timestamp() - server_clock;
    let resp = String::from_utf8(bytes.to_vec())?;
    log::trace!("Ping response: {}", resp);

    let parts = resp.split(';').collect::<Vec<_>>();
    if parts.len() < 9 {
        return Err(anyhow!("Invalid response"));
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

async fn safe_ip_check(addr: SocketAddr) -> Result<StatusPayload> {
    match single_ip_check(addr).await {
        Ok(status) => Ok(status),
        Err(e) => Err(anyhow!(
            "Protocol error in <{}:{}>: {}",
            addr.ip(),
            addr.port(),
            e
        )),
    }
}

async fn check_bedrock_server(addr_vec: Vec<SocketAddr>) -> Result<StatusPayload> {
    let mut set = JoinSet::new();

    for addr in addr_vec {
        set.spawn(safe_ip_check(addr));
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

pub struct BedrockQuery;

#[async_trait]
impl QueryModeHandler for BedrockQuery {
    async fn do_query(&self, addr: &str) -> Result<StatusPayload> {
        let (host, port) = sanitize_addr(addr, 19132)?;
        check_bedrock_server(resolve_addr(&host, port)).await
    }
}

impl BedrockQuery {
    pub fn new() -> BedrockQuery {
        BedrockQuery {}
    }
}
