//! Systems without an interface binding: every request is refused.

use std::io;

use super::types::IfaceState;

pub(super) fn state_of(_name: &str) -> IfaceState {
    IfaceState::Unsupported
}

pub(super) fn bind_socket(_sock: &socket2::Socket, _name: &str, _ipv6: bool) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
#[path = "os_other_tests.rs"]
mod tests;
