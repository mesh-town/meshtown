//! Linux and Android: `SO_BINDTODEVICE`.

use std::io;

use super::types::IfaceState;
use super::unix_flags;

pub(super) fn state_of(name: &str) -> IfaceState {
    unix_flags::flags_state(name)
}

/// Binds the socket to the device; a missing device is reported as `NotFound`.
pub(super) fn bind_socket(sock: &socket2::Socket, name: &str, _ipv6: bool) -> io::Result<()> {
    sock.bind_device(Some(name.as_bytes())).map_err(no_device_as_not_found)
}

fn no_device_as_not_found(e: io::Error) -> io::Error {
    match e.raw_os_error() {
        Some(libc::ENODEV) | Some(libc::ENXIO) => io::Error::new(io::ErrorKind::NotFound, e),
        _ => e,
    }
}

#[cfg(test)]
#[path = "os_linux_tests.rs"]
mod tests;
