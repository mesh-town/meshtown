//! Binding a UDP socket to an address on which someone else's socket already listens on `0.0.0.0`.
//!
//! What it does: `bind_shared` creates a socket with `SO_REUSEADDR` and binds it to a specific `ip:port`;
//! packets sent to this address are received by it, not by the "wide" socket (verified on macOS:
//! an exact address match beats `0.0.0.0`). This is how the proxy takes the addresses that the
//! server announces to the client for the direct path (`LossyLink::block_addr`).
//! What it does not have: IPv6 and other OSes — there `bind_shared` returns an `Unsupported` error.

use std::io;
use std::net::{SocketAddr, UdpSocket};

#[cfg(target_os = "macos")]
pub fn bind_shared(addr: SocketAddr) -> io::Result<UdpSocket> {
    let SocketAddr::V4(v4) = addr else {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "interception for IPv4 only",
        ));
    };
    let fd = mac::open_udp()?;
    mac::set_reuse_addr(&fd)?;
    mac::bind_v4(&fd, v4)?;
    let sock = UdpSocket::from(fd);
    sock.set_nonblocking(true)?;
    Ok(sock)
}

#[cfg(not(target_os = "macos"))]
pub fn bind_shared(_addr: SocketAddr) -> io::Result<UdpSocket> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "address interception is implemented for macOS only",
    ))
}

#[cfg(target_os = "macos")]
mod mac {
    use std::io;
    use std::mem::size_of;
    use std::net::SocketAddrV4;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    fn check(rc: libc::c_int) -> io::Result<()> {
        match rc {
            0 => Ok(()),
            _ => Err(io::Error::last_os_error()),
        }
    }

    pub fn open_udp() -> io::Result<OwnedFd> {
        let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    pub fn set_reuse_addr(fd: &OwnedFd) -> io::Result<()> {
        let one: libc::c_int = 1;
        let value = std::ptr::from_ref(&one).cast();
        let len = size_of::<libc::c_int>() as libc::socklen_t;
        check(unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_REUSEADDR,
                value,
                len,
            )
        })
    }

    pub fn bind_v4(fd: &OwnedFd, addr: SocketAddrV4) -> io::Result<()> {
        let sin = libc::sockaddr_in {
            sin_len: size_of::<libc::sockaddr_in>() as u8,
            sin_family: libc::AF_INET as libc::sa_family_t,
            sin_port: addr.port().to_be(),
            sin_addr: libc::in_addr {
                s_addr: u32::from_ne_bytes(addr.ip().octets()),
            },
            sin_zero: [0; 8],
        };
        let ptr = std::ptr::from_ref(&sin).cast();
        let len = size_of::<libc::sockaddr_in>() as libc::socklen_t;
        check(unsafe { libc::bind(fd.as_raw_fd(), ptr, len) })
    }
}
