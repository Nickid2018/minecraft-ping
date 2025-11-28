use anyhow::Result;
use regex::Regex;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::LazyLock;
use trust_dns_resolver::{AsyncResolver, system_conf};

const ADDRESS_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+):(\d+)$").expect("Compile regex failed!"));

pub async fn resolve_server_srv(addr: &str) -> Vec<String> {
    let (conf, opts) = system_conf::read_system_conf().expect("Could not read system conf");
    let raw_record = match AsyncResolver::tokio(conf, opts)
        .srv_lookup(format!("_minecraft._tcp.{}", addr))
        .await
    {
        Ok(record) => record,
        Err(e) => {
            log::debug!("Error getting srv records: {}", e);
            return Vec::new();
        }
    };

    let records = Vec::from_iter(raw_record.iter());
    log::debug!("{} srv record(s) found: ", records.len());
    records.iter().for_each(|srv| {
        log::debug!(
            "    {}:{} (Priority: {}, Weight: {})",
            srv.target(),
            srv.port(),
            srv.priority(),
            srv.weight()
        );
    });

    Vec::from_iter(
        records
            .iter()
            .map(|srv| format!("{}:{}", srv.target(), srv.port())),
    )
}

pub fn sanitize_addr(addr: &str, default_port: u16) -> Result<(String, u16)> {
    match ADDRESS_REGEX.captures(addr) {
        Some(captures) => Ok((captures[1].to_string(), captures[2].parse()?)),
        None => Ok((addr.to_string(), default_port)),
    }
}

pub fn resolve_addr(addr: &str, port: u16) -> Vec<SocketAddr> {
    match format!("{}:{}", addr, port).to_socket_addrs() {
        Ok(addrs_iter) => Vec::from_iter(addrs_iter),
        Err(e) => {
            log::debug!("Address resolving failed for {}:", addr);
            log::debug!("    {}", e);
            vec![]
        }
    }
}
