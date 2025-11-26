use std::net::SocketAddr;
use tokio::net::TcpSocket;

pub fn wrap_other<E: std::error::Error + Send + Sync + 'static>(e: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, e)
}

pub fn wrap_invalid<E: std::error::Error + Send + Sync + 'static>(e: E) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
}

pub fn make_tcp_socket(addr: &SocketAddr) -> std::io::Result<TcpSocket> {
    if addr.is_ipv4() {
        log::trace!("Using IPv4 socket to {}", addr);
        TcpSocket::new_v4()
    } else {
        log::trace!("Using IPv6 socket to {}", addr);
        TcpSocket::new_v6()
    }
}
