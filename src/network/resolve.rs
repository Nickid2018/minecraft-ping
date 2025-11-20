use regex::Regex;
use std::net::{SocketAddr, ToSocketAddrs};

#[cfg(target_os = "linux")]
use srv_rs::{SrvClient, resolver::libresolv::LibResolv};
#[cfg(target_os = "windows")]
use trust_dns_resolver::{AsyncResolver, system_conf};

#[cfg(target_os = "linux")]
pub async fn resolve_server_srv(addr: &str) -> Vec<String> {
    let client = SrvClient::<LibResolv>::new(format!("_minecraft._tcp.{}", addr));
    let record = match client.get_srv_records().await {
        Ok(record) => record,
        Err(e) => {
            log::debug!("Error getting srv records: {}", e);
            return Vec::new();
        }
    };

    log::debug!("{} srv record(s) found:", record.0.len());
    record.0.iter().for_each(|srv| {
        log::debug!(
            "    {}:{} (Priority: {}, Weight: {})",
            srv.target,
            srv.port,
            srv.priority,
            srv.weight
        );
    });

    Vec::from_iter(
        record
            .0
            .iter()
            .map(|srv| format!("{}:{}", srv.target, srv.port)),
    )
}

#[cfg(target_os = "windows")]
pub async fn resolve_server_srv(addr: &str) -> Vec<String> {
    let (conf, opts) = system_conf::read_system_conf().unwrap();
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

pub fn resolve_addr(addr: &str, default_port: u16) -> Option<Vec<SocketAddr>> {
    let addr_regex = Regex::new(r".+:\d+$").unwrap();
    let check_str = if addr_regex.is_match(&addr) {
        addr
    } else {
        &*format!("{}:{}", addr, default_port)
    };
    match check_str.to_socket_addrs() {
        Ok(addrs_iter) => Some(Vec::from_iter(addrs_iter)),
        Err(e) => {
            log::debug!("Address resolving failed for {}:", addr);
            log::debug!("    {}", e);
            None
        }
    }
}
