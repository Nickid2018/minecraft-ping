use crate::analyze::{MotdInfo, PlayerInfo, StatusPayload};
use crate::mode::QueryMode::JAVA;
use crate::mode::QueryModeHandler;
use crate::network::resolve::{resolve_addr, resolve_server_srv};
use crate::network::schema::{read_string, read_var_int_stream, write_var_int};
use crate::network::util::{io_timeout, now_timestamp};
use crate::util::make_tcp_socket;
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use clap::Args;
use serde_json::{Value, from_str};
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinSet;

async fn single_ip_check(addr: &SocketAddr, protocol: i32) -> std::io::Result<StatusPayload> {
    let time = Duration::from_secs(5);
    let ip_str = addr.ip().to_string();

    let socket = make_tcp_socket(addr)?;
    let mut stream = io_timeout(time, socket.connect(*addr), "Connection").await??;

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
    io_timeout(time, stream.read_exact(&mut handshake_recv), "Handshake").await??;
    let mut recv_buf = BytesMut::from(handshake_recv.as_slice());
    log::trace!("Handshake received from {}, length: {}", addr, handshake_recv.len());

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
    stream.write_i64(now_timestamp()).await?;
    stream.flush().await?;
    log::trace!("Ping request sent");

    let recv_pong = &mut [0; 10];
    io_timeout(time, stream.read_exact(recv_pong), "Ping receiving").await??;
    if recv_pong[0] != 9 || recv_pong[1] != 1 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "Invalid pong packet",
        ));
    }
    let server_clock = i64::from_be_bytes(recv_pong[2..10].try_into().expect("Recv failed"));
    let diff = now_timestamp() - server_clock;
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

async fn safe_ip_check(addr: SocketAddr, protocol: i32) -> std::io::Result<StatusPayload> {
    match single_ip_check(&addr, protocol).await {
        Ok(r) => Ok(r),
        Err(e) => {
            log::warn!("Failed to check available server ip {}: {}", addr, e);
            Err(e)
        }
    }
}

async fn check_java_server(
    addr_vec: Vec<SocketAddr>,
    protocol: i32,
) -> std::io::Result<StatusPayload> {
    let mut set = JoinSet::new();

    for addr in addr_vec {
        set.spawn(safe_ip_check(addr, protocol));
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

#[derive(Args, Debug)]
pub struct JavaModeArgs {
    /// Do not follow SRV redirection for Java query modes
    #[arg(long)]
    pub no_srv: bool,
    /// Simulate the protocol version of the client
    #[arg(long, default_value = "770")]
    pub protocol: i32,
}

pub struct JavaQuery<'a> {
    args: &'a JavaModeArgs,
}

pub async fn add_srv(addr: &str, addresses: &mut Vec<SocketAddr>) {
    let srv_res = resolve_server_srv(addr).await;
    let srv = srv_res
        .iter()
        .filter_map(|addr| resolve_addr(addr, 25565))
        .flatten();
    addresses.splice(0..0, srv);
}

#[async_trait]
impl QueryModeHandler for JavaQuery<'_> {
    async fn do_query(&self, addr: &str) -> std::io::Result<StatusPayload> {
        let je_res = resolve_addr(addr, 25565);
        let mut je_address = je_res.unwrap_or(vec![]);

        if !self.args.no_srv {
            add_srv(addr, &mut je_address).await;
        }

        check_java_server(je_address, self.args.protocol).await
    }
}

impl JavaQuery<'_> {
    pub fn new(args: &'_ JavaModeArgs) -> JavaQuery<'_> {
        JavaQuery { args }
    }
}
