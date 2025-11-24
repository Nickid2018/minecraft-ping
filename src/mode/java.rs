use crate::analyze::{MotdInfo, PlayerInfo, StatusPayload};
use crate::mode::QueryModeHandler;
use crate::network::resolve::{resolve_addr, resolve_server_srv};
use crate::network::schema::{read_string, read_var_int_stream, write_var_int};
use crate::network::util;
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use clap::Args;
use serde_json::{Value, from_str};
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpSocket;
use tokio::time::timeout;
use crate::mode::QueryMode::JAVA;

async fn single_ip_check(addr: &SocketAddr, protocol: i32) -> std::io::Result<StatusPayload> {
    let timeout_time = Duration::from_secs(5);
    let ip_str = addr.ip().to_string();

    let socket = if addr.is_ipv4() {
        log::trace!("Using IPv4 socket to {}", addr);
        TcpSocket::new_v4()?
    } else {
        log::trace!("Using IPv6 socket to {}", addr);
        TcpSocket::new_v6()?
    };
    let mut stream = timeout(timeout_time, socket.connect(*addr)).await??;

    let mut handshake = vec![0];
    write_var_int(&mut handshake, protocol); // protocol_version
    write_var_int(&mut handshake, ip_str.len() as i32); // host string
    handshake.extend_from_slice(ip_str.as_bytes());
    handshake.put_u16(addr.port()); // port
    handshake.push(1); // to status
    let mut handshake_packed: Vec<u8> = vec![];
    write_var_int(&mut handshake_packed, handshake.len() as i32);
    handshake_packed.extend_from_slice(handshake.as_slice());
    stream.write_all(&handshake_packed).await?;
    stream.write(&[1, 0]).await?; // status
    stream.flush().await?;
    log::trace!("Handshake sent");

    let handshake_recv_len = read_var_int_stream(&mut stream).await?;
    let mut handshake_recv = vec![0; handshake_recv_len as usize];
    timeout(timeout_time, stream.read_exact(&mut handshake_recv)).await??;
    let mut recv_buf = BytesMut::from(handshake_recv.as_slice());
    log::trace!("Handshake received, length: {}", handshake_recv.len());

    if recv_buf.remaining() == 0 || recv_buf.get_u8() != 0 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Invalid received packet id",
        ));
    }
    let json_str = read_string(&mut recv_buf)?;
    if recv_buf.remaining() != 0 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "Packet length is invalid: Trailing {} bytes",
                recv_buf.remaining()
            ),
        ));
    }
    log::trace!("Got json: {}", json_str);
    let mut decoded: Value = from_str(&json_str).map_err(crate::util::wrap_invalid)?;

    stream.write(&[9, 1]).await?; // ping_request
    stream.write_i64(util::now_timestamp()).await?;
    stream.flush().await?;
    log::trace!("Ping request sent");

    let recv_pong = &mut [0; 10];
    timeout(timeout_time, stream.read_exact(recv_pong)).await??;
    if recv_pong[0] != 9 || recv_pong[1] != 1 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Invalid pong packet",
        ));
    }
    let server_clock = i64::from_be_bytes(recv_pong[2..10].try_into().expect("Recv failed"));
    let diff = util::now_timestamp() - server_clock;
    log::trace!("Got ping time: {}", diff);

    let players = decoded["players"].take();
    let player_count = players["online"].as_i64();
    let max_players = players["max"].as_i64();
    let players = players["sample"].as_array().map(|v| {
        Vec::from_iter(v.iter().map(|e| PlayerInfo {
            id: e["name"].as_str().unwrap_or("<UNKNOWN NAME>").to_string(),
            uuid: e["id"].as_str().unwrap_or("<UNKNOWN ID>").to_string(),
        }))
    });

    let desc = decoded["description"].take();
    let motd = if desc.is_string() {
        Some(MotdInfo::String(desc.as_str().unwrap_or("").to_string()))
    } else if desc.is_object() {
        Some(MotdInfo::Component(desc))
    } else {
        None
    };

    let version = decoded["version"].take();
    let protocol = version["protocol"].as_i64();
    let version_name = version["name"].as_str().map(|v| v.to_string());

    let favicon = decoded["favicon"].take().as_str().map(|v| v.to_string());

    Ok(StatusPayload {
        mode: JAVA,
        ping: diff,
        max_players,
        player_count,
        players,
        motd,
        protocol,
        version_name,
        favicon,
        full_extra: Some(decoded),
    })
}

async fn check_java_server(
    addr_vec: &Vec<SocketAddr>,
    protocol: i32,
) -> std::io::Result<StatusPayload> {
    for addr in addr_vec {
        match single_ip_check(&addr, protocol).await {
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

#[derive(Args, Debug)]
pub struct JavaModeArgs {
    /// Do not follow SRV redirection for Java and Legacy query mode
    #[arg(long)]
    pub no_srv: bool,
    /// Simulate the protocol version of the client
    #[arg(long, default_value = "770")]
    pub protocol: i32,
}

pub struct JavaQuery<'a> {
    args: &'a JavaModeArgs,
}

#[async_trait]
impl QueryModeHandler for JavaQuery<'_> {
    async fn do_query(&self, addr: &str) -> std::io::Result<StatusPayload> {
        let mut je_res = resolve_addr(addr, 25565);
        let je_address = je_res.get_or_insert_default();

        if !self.args.no_srv {
            let srv_res = resolve_server_srv(addr).await;
            let srv = srv_res
                .iter()
                .filter_map(|addr| resolve_addr(addr, 25565))
                .flatten();
            je_address.splice(0..0, srv);
        }

        check_java_server(je_address, self.args.protocol).await
    }
}

impl JavaQuery<'_> {
    pub fn new(args: &'_ JavaModeArgs) -> JavaQuery<'_> {
        JavaQuery { args }
    }
}
