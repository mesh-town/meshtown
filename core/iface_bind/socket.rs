//! Sockets bound to a named interface: the operating-system specific part lives in `os`.

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::{TcpSocket, TcpStream};

use super::os;
use super::types::{BindError, IfaceState};

/// State of the named interface.
pub fn state_of(name: &str) -> IfaceState {
    os::state_of(name)
}

/// Checks that connections can be bound to the interface right now: it exists, it is up,
/// and a throwaway UDP socket accepts the binding (nothing is sent).
pub fn check(name: &str) -> Result<(), BindError> {
    match state_of(name) {
        IfaceState::Up => {}
        IfaceState::Down => return Err(BindError::Down(name.to_string())),
        IfaceState::Missing => return Err(BindError::Missing(name.to_string())),
        IfaceState::Unsupported => return Err(BindError::Unsupported),
    }
    probe(name).map_err(|e| classify(name, e))
}

fn probe(name: &str) -> io::Result<()> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    os::bind_socket(&sock, name, false)
}

/// Translates an OS error of the binding into a facade error.
pub(super) fn classify(name: &str, e: io::Error) -> BindError {
    let name = name.to_string();
    match e.kind() {
        io::ErrorKind::PermissionDenied => BindError::NoPermission(name),
        io::ErrorKind::NotFound => BindError::Missing(name),
        io::ErrorKind::Unsupported => BindError::Unsupported,
        _ => BindError::Os(name, e),
    }
}

fn domain_of(addr: &SocketAddr) -> Domain {
    Domain::for_address(*addr)
}

fn unspecified(ipv6: bool) -> SocketAddr {
    match ipv6 {
        true => SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)),
        false => SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
    }
}

fn bound_error(name: &str, e: io::Error) -> io::Error {
    let kind = e.kind();
    io::Error::new(kind, classify(name, e))
}

/// Opens a TCP connection; with an interface name the socket is bound to it before connecting.
pub async fn connect_tcp(addr: SocketAddr, iface: Option<Arc<str>>) -> io::Result<TcpStream> {
    let Some(name) = iface else {
        return TcpStream::connect(addr).await;
    };
    let sock = Socket::new(domain_of(&addr), Type::STREAM, Some(Protocol::TCP))?;
    os::bind_socket(&sock, &name, addr.is_ipv6()).map_err(|e| bound_error(&name, e))?;
    sock.set_nonblocking(true)?;
    TcpSocket::from_std_stream(sock.into()).connect(addr).await
}

/// Creates a UDP socket for talking to `dest`; with an interface name it is bound to it.
/// The caller sets the non-blocking mode.
pub fn bind_udp(dest: SocketAddr, iface: Option<&str>) -> io::Result<std::net::UdpSocket> {
    let local = unspecified(dest.is_ipv6());
    let Some(name) = iface else {
        return std::net::UdpSocket::bind(local);
    };
    let sock = Socket::new(domain_of(&dest), Type::DGRAM, Some(Protocol::UDP))?;
    os::bind_socket(&sock, name, dest.is_ipv6()).map_err(|e| bound_error(name, e))?;
    sock.bind(&local.into())?;
    Ok(sock.into())
}

#[cfg(test)]
#[path = "socket_tests.rs"]
mod tests;
