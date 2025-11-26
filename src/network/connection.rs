use crate::network::resolve::resolve_addr;
use crate::network::util::generic_timeout;
use anyhow::{Result, anyhow};
use clap::Args;
use proxied::{NetworkTarget, Proxy};
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::net::{TcpSocket, TcpStream};
use tokio::task::JoinSet;

#[derive(Args, Debug)]
pub struct ProxySettings {
    /// Make query through proxy (environment variable `ALL_PROXY` also works)
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
        succeed.push(
            generic_timeout(time, proxy_tcp(proxy, addr, port), "Connection".to_string()).await?,
        );
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
