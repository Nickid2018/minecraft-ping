use crate::analyze::{MotdInfo, StatusPayload};
use crate::mode::QueryMode::BEDROCK;
use crate::mode::QueryModeHandler;
use crate::network::connection::{ProxyableUdpSocket, UdpTarget, udp_socket};
use crate::network::resolve::sanitize_addr;
use crate::network::util::{generic_timeout, now_timestamp};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use serde_json::json;
use std::time::Duration;
use tokio::task::JoinSet;

const MAGIC_HIGH: u64 = 0x00ffff00fefefefeu64;
const MAGIC_LOW: u64 = 0xfdfdfdfd12345678u64;

async fn single_ip_check(addr: &UdpTarget, socket: ProxyableUdpSocket) -> Result<StatusPayload> {
    let timeout_time = Duration::from_secs(5);

    let timestamp = now_timestamp();
    let mut packet = Vec::from([1u8]);
    packet.put_i64(timestamp);
    packet.put_u64(MAGIC_HIGH);
    packet.put_u64(MAGIC_LOW);
    packet.put_u16(0);
    socket.send_to(&packet, addr).await?;
    log::trace!("Sent Unconnected Ping packet");

    let mut recv_buf = [0u8; 1024];
    let recv = generic_timeout(timeout_time, socket.recv_from(&mut recv_buf), "Recv").await?;
    log::trace!("Received response from {}", addr);

    let mut bytes = BytesMut::from(&recv_buf[..recv]);
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
    if str_len as usize != recv - 35 {
        return Err(anyhow!("Invalid string length"));
    }

    let ping = now_timestamp() - server_clock;
    let resp = String::from_utf8(bytes.to_vec())?;
    log::trace!("Ping response: {}", resp);

    let parts = resp.split(';').collect::<Vec<_>>();
    if parts.len() < 9 {
        return Err(anyhow!("Invalid response"));
    }

    Ok(StatusPayload {
        mode: BEDROCK,
        ping,
        max_players: Some(parts[5].parse()?),
        player_count: Some(parts[4].parse()?),
        players: None,
        motd: Some(MotdInfo::String(format!("{}\n{}", parts[1], parts[7]))),
        protocol: Some(parts[2].parse()?),
        version_name: Some(parts[3].to_string()),
        favicon: None,
        full_extra: Some(json!({"server_guid": server_guid, "game_mode": parts[8].to_string()})),
    })
}

async fn safe_ip_check(addr: UdpTarget, socket: ProxyableUdpSocket) -> Result<StatusPayload> {
    match single_ip_check(&addr, socket).await {
        Ok(status) => Ok(status),
        Err(e) => Err(anyhow!("Protocol error in <{}>: {}", addr, e)),
    }
}

pub struct BedrockQuery;

#[async_trait]
impl QueryModeHandler for BedrockQuery {
    async fn do_query(&self, addr: &str) -> Result<StatusPayload> {
        let (host, port) = sanitize_addr(addr, 19132)?;
        let socks = udp_socket(&host, port, Duration::new(5, 0)).await?;
        let mut set = JoinSet::new();

        for sock in socks {
            set.spawn(safe_ip_check(sock.0, sock.1));
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
}

impl BedrockQuery {
    pub fn new() -> BedrockQuery {
        BedrockQuery {}
    }
}
