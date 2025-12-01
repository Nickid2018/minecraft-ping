use crate::network::resolve::resolve_addr;
use crate::network::util::generic_timeout;
use anyhow::{Result, anyhow};
use async_http_proxy::{
    http_connect_tokio as http_proxy, http_connect_tokio_with_basic_auth as http_proxy_auth,
};
use clap::Args;
use fast_socks5::client::{Config, Socks5Datagram, Socks5Stream};
use fast_socks5::util::target_addr::TargetAddr;
use fast_socks5::{AuthenticationMethod, Socks5Command};
use regex_lite::Regex;
use std::cmp::PartialEq;
use std::fmt::Display;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;
use tokio::net::{TcpSocket, TcpStream, UdpSocket};
use tokio::task::JoinSet;

#[derive(Args, Debug)]
pub struct ProxySettings {
    /// Make query through proxy (environment variable `ALL_PROXY` also works)
    /// [Note: Cannot proxy UDP packets when using http proxies]
    #[arg(long)]
    pub proxy: Option<String>,
}

const PROXY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(http|https|socks5)://(?:(.+):(.+)@)?(.+?)(?::(\d+))?$")
        .expect("Compile regex failed")
});

#[derive(Debug, Copy, Clone, PartialEq)]
enum ProxyType {
    Http,
    Socks5,
}

static PROXY_SETTING: OnceLock<(ProxyType, SocketAddr)> = OnceLock::new();
static PROXY_CRED: OnceLock<(String, String)> = OnceLock::new();

pub fn setup_proxy(proxy_settings: &ProxySettings) {
    if let Some(proxy) = proxy_settings.proxy.as_ref() {
        if let Some(matches) = PROXY_REGEX.captures(proxy) {
            let scheme = matches.get(1).expect("Should have scheme").as_str();
            let proxy_type = match scheme {
                "socks5" => ProxyType::Socks5,
                _ => ProxyType::Http,
            };

            let port = if let Some(port) = matches.get(5) {
                match u16::from_str(port.as_str()) {
                    Ok(port) => port,
                    Err(e) => {
                        log::warn!("Proxy port is invalid: {}", e);
                        return;
                    }
                }
            } else {
                match scheme {
                    "socks5" => 1080,
                    "http" => 80,
                    _ => 443,
                }
            };

            if matches.get(2).is_some() {
                PROXY_CRED
                    .set((
                        matches
                            .get(2)
                            .expect("Should have user")
                            .as_str()
                            .to_string(),
                        matches
                            .get(3)
                            .expect("Should have password")
                            .as_str()
                            .to_string(),
                    ))
                    .expect("Should set proxy credentials");
            };

            if let Some(addr) =
                resolve_addr(matches.get(4).expect("Should have host").as_str(), port).get(0)
            {
                PROXY_SETTING
                    .set((proxy_type, *addr))
                    .expect("Should be set");
            } else {
                log::warn!("Proxy cannot be resolved");
            }
        } else {
            log::warn!("Proxy setting is invalid");
        }
    }
}

async fn setup_proxy_stream() -> Result<TcpStream> {
    if let Some((_, addr)) = PROXY_SETTING.get() {
        log::debug!("Setup proxy stream for {}", addr);
        let stream: TcpStream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        stream.set_linger(None)?;
        Ok(stream)
    } else {
        Err(anyhow!("Proxy setting is invalid"))
    }
}

async fn proxy_tcp(host: &str, port: u16) -> Result<TcpStream> {
    let mut stream: TcpStream = setup_proxy_stream().await?;
    if let Some(proxy_type) = PROXY_SETTING.get().map(|p| p.0) {
        match proxy_type {
            ProxyType::Http => {
                match PROXY_CRED.get() {
                    Some((user, p)) => http_proxy_auth(&mut stream, host, port, user, p).await,
                    None => http_proxy(&mut stream, host, port).await,
                }?;
            }
            ProxyType::Socks5 => {
                let auth = PROXY_CRED
                    .get()
                    .map(|(u, p)| AuthenticationMethod::Password {
                        username: u.clone(),
                        password: p.clone(),
                    });
                let config = Config::default();
                let mut proxied = Socks5Stream::use_stream(&mut stream, auth, config).await?;
                proxied
                    .request(
                        Socks5Command::TCPConnect,
                        TargetAddr::Domain(host.to_string(), port),
                    )
                    .await?;
            }
        }
        Ok(stream)
    } else {
        Err(anyhow!("Proxy setting is invalid"))
    }
}

async fn no_proxy_tcp0(addr: SocketAddr) -> Result<TcpStream> {
    let socket = if addr.is_ipv4() {
        log::trace!("Using IPv4 socket to {}", addr);
        TcpSocket::new_v4()?
    } else {
        log::trace!("Using IPv6 socket to {}", addr);
        TcpSocket::new_v6()?
    };
    Ok(socket.connect(addr).await?)
}

async fn no_proxy_tcp(addr: SocketAddr) -> Result<TcpStream> {
    match no_proxy_tcp0(addr).await {
        Ok(stream) => Ok(stream),
        Err(e) => Err(anyhow!("<{}:{}>: {}", addr.ip(), addr.port(), e)),
    }
}

pub async fn connect_tcp(addr: &str, port: u16, time: Duration) -> Result<Vec<TcpStream>> {
    let mut succeed = vec![];
    if PROXY_SETTING.get().is_some() {
        succeed.push(generic_timeout(time, proxy_tcp(addr, port), "Connection").await?);
    } else {
        let addrs = resolve_addr(addr, port);
        let mut join_set = JoinSet::new();
        for addr in addrs {
            let name = format!("Connection {}:{}", addr.ip(), addr.port());
            join_set.spawn(generic_timeout(time, no_proxy_tcp(addr), name));
        }
        while let Some(join_res) = join_set.join_next().await {
            if let Ok(res) = join_res {
                match res {
                    Ok(stream) => succeed.push(stream),
                    Err(e) => log::warn!("{}", e),
                }
            }
        }
    }
    Ok(succeed)
}

pub struct UdpTarget {
    host: String,
    port: u16,
    addr: Option<SocketAddr>,
}

impl Display for UdpTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

enum ProxyableUdpSocketType {
    Proxied(Socks5Datagram<TcpStream>),
    NotProxied(UdpSocket),
}

pub struct ProxyableUdpSocket {
    sock: ProxyableUdpSocketType,
}

impl ProxyableUdpSocket {
    pub async fn send_to(&self, data: &[u8], addr: &UdpTarget) -> Result<usize> {
        match &self.sock {
            ProxyableUdpSocketType::Proxied(proxied) => Ok(proxied
                .send_to(data, (addr.host.as_str(), addr.port))
                .await?),
            ProxyableUdpSocketType::NotProxied(sock) => Ok(sock
                .send_to(data, addr.addr.ok_or(anyhow!("Should exist socket addr"))?)
                .await?),
        }
    }

    pub async fn recv_from(&self, data: &mut [u8]) -> Result<usize> {
        match &self.sock {
            ProxyableUdpSocketType::Proxied(proxied) => Ok(proxied.recv_from(data).await?.0),
            ProxyableUdpSocketType::NotProxied(sock) => Ok(sock.recv_from(data).await?.0),
        }
    }
}

async fn no_proxy_udp0(addr: SocketAddr) -> Result<(UdpTarget, UdpSocket)> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(addr).await?;
    Ok((
        UdpTarget {
            host: addr.ip().to_string(),
            port: addr.port(),
            addr: Some(addr),
        },
        socket,
    ))
}

async fn no_proxy_udp(addr: SocketAddr) -> Result<(UdpTarget, UdpSocket)> {
    match no_proxy_udp0(addr).await {
        Ok((sock, target)) => Ok((sock, target)),
        Err(e) => Err(anyhow!("<{}:{}>: {}", addr.ip(), addr.port(), e)),
    }
}

pub async fn udp_socket(
    addr: &str,
    port: u16,
    time: Duration,
) -> Result<Vec<(UdpTarget, ProxyableUdpSocket)>> {
    let mut succeed: Vec<(UdpTarget, _)> = vec![];
    if let Some((proxy_type, _)) = PROXY_SETTING.get()
        && *proxy_type == ProxyType::Socks5
    {
        let stream: TcpStream = setup_proxy_stream().await?;
        let proxied_datagram = if let Some(cred) = PROXY_CRED.get() {
            Socks5Datagram::bind_with_password(stream, "0.0.0.0:0", &cred.0, &cred.1).await?
        } else {
            Socks5Datagram::bind(stream, "0.0.0.0:0").await?
        };
        succeed.push((
            UdpTarget {
                host: addr.to_string(),
                port,
                addr: None,
            },
            ProxyableUdpSocket {
                sock: ProxyableUdpSocketType::Proxied(proxied_datagram),
            },
        ));
    } else {
        let addrs = resolve_addr(addr, port);
        let mut join_set = JoinSet::new();
        for addr in addrs {
            if addr.is_ipv6() {
                log::trace!("Skip IPv6 address {}", addr);
                continue;
            }
            let name = format!("Connection {}:{}", addr.ip(), addr.port());
            join_set.spawn(generic_timeout(time, no_proxy_udp(addr), name));
        }
        while let Some(join_res) = join_set.join_next().await {
            if let Ok(res) = join_res {
                match res {
                    Ok((target, sock)) => succeed.push((
                        target,
                        ProxyableUdpSocket {
                            sock: ProxyableUdpSocketType::NotProxied(sock),
                        },
                    )),
                    Err(e) => log::warn!("{}", e),
                }
            }
        }
    }
    Ok(succeed)
}
