//! Windows: `IP_UNICAST_IF` / `IPV6_UNICAST_IF` by interface index.
//! The name is the interface's friendly name (the alias shown in network settings).

use std::io;
use std::os::windows::io::AsRawSocket;

use windows_sys::Win32::NetworkManagement::IpHelper::{
    ConvertInterfaceAliasToLuid, ConvertInterfaceLuidToIndex, GetIfEntry2, MIB_IF_ROW2,
};
use windows_sys::Win32::NetworkManagement::Ndis::{IfOperStatusUp, NET_LUID_LH};
use windows_sys::Win32::Networking::WinSock::{
    setsockopt, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF, IP_UNICAST_IF, SOCKET_ERROR,
};

use super::types::IfaceState;

const NO_ERROR: u32 = 0;

/// Looks the alias up; an unknown alias is `NotFound`.
fn luid_of(name: &str) -> io::Result<NET_LUID_LH> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: an all-zero LUID is a valid value of this plain union.
    let mut luid: NET_LUID_LH = unsafe { std::mem::zeroed() };
    // SAFETY: `wide` is NUL-terminated and `luid` is a valid out-pointer.
    let rc = unsafe { ConvertInterfaceAliasToLuid(wide.as_ptr(), &mut luid) };
    match rc {
        NO_ERROR => Ok(luid),
        _ => Err(io::Error::new(io::ErrorKind::NotFound, "no such interface")),
    }
}

fn index_of(name: &str) -> io::Result<u32> {
    let luid = luid_of(name)?;
    let mut index = 0u32;
    // SAFETY: `luid` and `index` are valid pointers for the duration of the call.
    let rc = unsafe { ConvertInterfaceLuidToIndex(&luid, &mut index) };
    match rc {
        NO_ERROR => Ok(index),
        code => Err(io::Error::from_raw_os_error(code as i32)),
    }
}

pub(super) fn state_of(name: &str) -> IfaceState {
    let Ok(luid) = luid_of(name) else {
        return IfaceState::Missing;
    };
    // SAFETY: an all-zero row is a valid starting value; only the LUID key is set before the query.
    let mut row: MIB_IF_ROW2 = unsafe { std::mem::zeroed() };
    row.InterfaceLuid = luid;
    // SAFETY: `row` is a valid, initialised row with its key set.
    let rc = unsafe { GetIfEntry2(&mut row) };
    match (rc, row.OperStatus == IfOperStatusUp) {
        (NO_ERROR, true) => IfaceState::Up,
        (NO_ERROR, false) => IfaceState::Down,
        _ => IfaceState::Missing,
    }
}

fn set_unicast_if(sock: &socket2::Socket, level: i32, option: i32, value: u32) -> io::Result<()> {
    let raw = sock.as_raw_socket() as usize;
    // SAFETY: `raw` is a live socket owned by `sock`; the pointer and length describe `value`.
    let rc = unsafe { setsockopt(raw, level, option, (&value as *const u32).cast::<u8>(), 4) };
    match rc {
        SOCKET_ERROR => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

/// IPv4 takes the index in network byte order, IPv6 in host order.
pub(super) fn bind_socket(sock: &socket2::Socket, name: &str, ipv6: bool) -> io::Result<()> {
    let index = index_of(name)?;
    match ipv6 {
        true => set_unicast_if(sock, IPPROTO_IPV6, IPV6_UNICAST_IF, index),
        false => set_unicast_if(sock, IPPROTO_IP, IP_UNICAST_IF, index.to_be()),
    }
}

#[cfg(test)]
#[path = "os_windows_tests.rs"]
mod tests;
