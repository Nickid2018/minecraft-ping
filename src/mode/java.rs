use crate::analyze::{MotdInfo, PlayerInfo, StatusPayload};
use crate::mode::QueryMode::JAVA;
use crate::mode::QueryModeHandler;
use crate::network::connection::connect_tcp;
use crate::network::resolve::{resolve_server_srv, sanitize_addr};
use crate::network::schema::{read_string, read_var_int_stream, write_var_int};
use crate::network::util::{io_timeout, now_timestamp};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use clap::Args;
use serde_json::{Value, from_str};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinSet;

async fn single_ip_check(
    addr: &str,
    port: u16,
    stream: &mut TcpStream,
    protocol: i32,
) -> Result<StatusPayload> {
    let time = Duration::from_secs(5);

    let mut handshake = vec![0];
    write_var_int(&mut handshake, protocol); // protocol_version
    write_var_int(&mut handshake, addr.len() as i32); // host string
    handshake.extend_from_slice(addr.as_bytes());
    handshake.put_u16(port); // port
    handshake.push(1); // to status
    let mut handshake_packed: Vec<u8> = vec![];
    write_var_int(&mut handshake_packed, handshake.len() as i32);
    handshake_packed.extend_from_slice(handshake.as_slice());
    stream.write_all(&handshake_packed).await?;
    stream.write(&[1, 0]).await?; // status
    stream.flush().await?;
    log::trace!("Handshake sent");

    let handshake_recv_len = read_var_int_stream(stream).await?;
    let mut handshake_recv = vec![0; handshake_recv_len as usize];
    io_timeout(time, stream.read_exact(&mut handshake_recv), "Handshake").await?;
    let mut recv_buf = BytesMut::from(handshake_recv.as_slice());
    log::trace!(
        "Handshake received from {}, length: {}",
        addr,
        handshake_recv.len()
    );

    if recv_buf.remaining() == 0 || recv_buf.get_u8() != 0 {
        return Err(anyhow!("Invalid received packet id"));
    }
    let json_str = read_string(&mut recv_buf)?;
    if recv_buf.remaining() != 0 {
        return Err(anyhow!(
            "Packet length is invalid: Trailing {} bytes",
            recv_buf.remaining()
        ));
    }
    log::trace!("Got json: {}", json_str);
    let mut decoded: Value = from_str(&json_str)?;

    stream.write(&[9, 1]).await?; // ping_request
    stream.write_i64(now_timestamp()).await?;
    stream.flush().await?;
    log::trace!("Ping request sent");

    let recv_pong = &mut [0; 10];
    io_timeout(time, stream.read_exact(recv_pong), "Ping receiving").await?;
    if recv_pong[0] != 9 || recv_pong[1] != 1 {
        return Err(anyhow!("Invalid pong packet"));
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

async fn safe_ip_check(
    addr: String,
    port: u16,
    mut stream: TcpStream,
    protocol: i32,
) -> Result<StatusPayload> {
    match single_ip_check(&addr, port, &mut stream, protocol).await {
        Ok(resp) => Ok(resp),
        Err(e) => Err(anyhow!("Protocol error in <{}:{}>: {}", addr, port, e)),
    }
}

async fn check_java_server(
    addr: &str,
    port: u16,
    streams: Vec<TcpStream>,
    protocol: i32,
) -> Result<StatusPayload> {
    let mut set = JoinSet::new();

    for stream in streams {
        set.spawn(safe_ip_check(addr.to_string(), port, stream, protocol));
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

#[async_trait]
impl QueryModeHandler for JavaQuery<'_> {
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
                    match check_java_server(&host, port, streams, self.args.protocol).await {
                        Ok(status) => return Ok(status),
                        Err(e) => log::warn!("Failed to check <{}:{}>: {}", host, port, e),
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

impl JavaQuery<'_> {
    pub fn new(args: &'_ JavaModeArgs) -> JavaQuery<'_> {
        JavaQuery { args }
    }
}
