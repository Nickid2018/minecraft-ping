use regex::Regex;
use std::net::{SocketAddr, ToSocketAddrs};

use trust_dns_resolver::{AsyncResolver, system_conf};

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
