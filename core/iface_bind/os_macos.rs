//! macOS: `IP_BOUND_IF` / `IPV6_BOUND_IF` by interface index.

use std::ffi::CString;
use std::io;
use std::num::NonZeroU32;

use super::types::IfaceState;
use super::unix_flags;

pub(super) fn state_of(name: &str) -> IfaceState {
    unix_flags::flags_state(name)
}

/// Resolves the interface index; zero (no such interface) is reported as `NotFound`.
fn index_of(name: &str) -> io::Result<NonZeroU32> {
    let c_name = CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "interface name contains NUL"))?;
    // SAFETY: `c_name` is a valid NUL-terminated string for the duration of the call.
    let index = unsafe { libc::if_nametoindex(c_name.as_ptr()) };
    NonZeroU32::new(index)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such interface"))
}

pub(super) fn bind_socket(sock: &socket2::Socket, name: &str, ipv6: bool) -> io::Result<()> {
    let index = Some(index_of(name)?);
    match ipv6 {
        true => sock.bind_device_by_index_v6(index),
        false => sock.bind_device_by_index_v4(index),
    }
}

#[cfg(test)]
#[path = "os_macos_tests.rs"]
mod tests;
