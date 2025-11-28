use crate::network::resolve::resolve_addr;
use crate::network::util::generic_timeout;
use anyhow::{Result, anyhow};
use clap::Args;
use fast_socks5::client::Socks5Datagram;
use proxied::{NetworkTarget, Proxy, ProxyKind};
use std::fmt::Display;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::OnceLock;
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

static PROXY: OnceLock<Proxy> = OnceLock::new();

pub fn setup_proxy(proxy_settings: &ProxySettings) {
    if let Some(proxy) = proxy_settings.proxy.as_ref() {
        let sanitized = proxy.replace("//", "");
        match sanitized.parse() {
            Ok(proxy) => {
                PROXY.set(proxy).expect("Set proxy");
            }
            Err(e) => {
                log::warn!("Proxy setting is invalid: {}", e);
            }
        }
    }
}

async fn proxy_tcp(proxy: &Proxy, host: &str, port: u16) -> Result<TcpStream> {
    log::trace!("Using proxy {} to {}:{}", proxy, host, port);
    Ok(proxy
        .connect_tcp(NetworkTarget::Domain {
            domain: host.to_string(),
            port,
        })
        .await?)
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
    if let Some(proxy) = PROXY.get() {
        succeed.push(generic_timeout(time, proxy_tcp(proxy, addr, port), "Connection").await?);
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
    if let Some(proxy) = PROXY.get()
        && proxy.kind == ProxyKind::Socks5
    {
        let resolved_addr = match &proxy.is_dns_addr() {
            true => *resolve_addr(&proxy.addr, proxy.port)
                .get(0)
                .ok_or(anyhow!("Unable to resolve proxy address"))?,
            false => SocketAddr::from_str(&format!("{}:{}", proxy.addr, proxy.port))?,
        };

        let stream: TcpStream = TcpStream::connect(resolved_addr).await?;
        stream.set_nodelay(true)?;
        stream.set_linger(None)?;

        let proxied_datagram = if let Some(cred) = &proxy.creds {
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
