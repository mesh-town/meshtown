//! Interface state from the `getifaddrs` flags (Linux, Android, macOS).

use std::ffi::CStr;

use super::types::IfaceState;

/// Pure decision from the collected flags of all entries with the interface name.
pub(super) fn state_from(found: bool, up: bool, running: bool) -> IfaceState {
    match (found, up && running) {
        (false, _) => IfaceState::Missing,
        (true, true) => IfaceState::Up,
        (true, false) => IfaceState::Down,
    }
}

/// Walks the interface list and reports the state of `name`.
pub(super) fn flags_state(name: &str) -> IfaceState {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: `head` is a valid out-pointer; on success the list is released below.
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return IfaceState::Missing;
    }
    let (mut found, mut up, mut running) = (false, false, false);
    let mut cur = head;
    while !cur.is_null() {
        // SAFETY: `cur` is a non-null node of the list returned by `getifaddrs`,
        // which stays valid until `freeifaddrs`.
        let entry = unsafe { &*cur };
        if entry_name_is(entry, name) {
            found = true;
            up |= entry.ifa_flags & libc::IFF_UP as libc::c_uint != 0;
            running |= entry.ifa_flags & libc::IFF_RUNNING as libc::c_uint != 0;
        }
        cur = entry.ifa_next;
    }
    // SAFETY: `head` came from a successful `getifaddrs` and is freed exactly once.
    unsafe { libc::freeifaddrs(head) };
    state_from(found, up, running)
}

fn entry_name_is(entry: &libc::ifaddrs, name: &str) -> bool {
    if entry.ifa_name.is_null() {
        return false;
    }
    // SAFETY: `ifa_name` is a NUL-terminated string owned by the list.
    let raw = unsafe { CStr::from_ptr(entry.ifa_name) };
    raw.to_bytes() == name.as_bytes()
}

#[cfg(test)]
#[path = "unix_flags_tests.rs"]
mod tests;
