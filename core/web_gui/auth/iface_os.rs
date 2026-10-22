//! Reading interfaces from the OS: a view cached for 2 seconds, the default route, packet forwarding.

use std::net::{Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::iface_gate::{IfaceView, ViewAddr};

const VIEW_TTL: Duration = Duration::from_secs(2);

type Cached = Option<(Instant, Arc<IfaceView>)>;
static VIEW_CACHE: Mutex<Cached> = Mutex::new(None);

/// The current interface view; one cache per process, an OS error gives an empty view.
pub(crate) fn current_view() -> Arc<IfaceView> {
    let mut slot = VIEW_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, view)) = slot.as_ref().filter(|(at, _)| at.elapsed() < VIEW_TTL) {
        return view.clone();
    }
    let fresh = Arc::new(read_view());
    *slot = Some((Instant::now(), fresh.clone()));
    fresh
}

fn read_view() -> IfaceView {
    let all = if_addrs::get_if_addrs().unwrap_or_default();
    let v4 = all.into_iter().filter_map(|i| match i.addr {
        if_addrs::IfAddr::V4(a) if !a.ip.is_loopback() => {
            Some((i.name, a.ip, a.netmask, a.broadcast.is_some()))
        }
        _ => None,
    });
    view_from(v4.collect(), default_ipv4(), forwarding())
}

/// Pure assembly of the view: (name, address, mask, whether it has broadcast).
pub(crate) fn view_from(
    addrs: Vec<(String, Ipv4Addr, Ipv4Addr, bool)>,
    default_ip: Option<Ipv4Addr>,
    forwarding: bool,
) -> IfaceView {
    let addrs = addrs
        .into_iter()
        .map(|(name, ip, mask, broadcast)| ViewAddr {
            name,
            ip,
            prefix: u32::from(mask).count_ones() as u8,
            broadcast,
        })
        .collect();
    IfaceView {
        addrs,
        default_ip,
        forwarding,
    }
}

/// Local address of a UDP socket "connected" to `192.0.2.1:9` (sends no packets), without a filter.
pub(crate) fn default_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("192.0.2.1:9").ok()?;
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        std::net::IpAddr::V6(_) => None,
    }
}

#[cfg(any(test, target_os = "linux"))]
pub(crate) fn forwarding_from_text(s: &str) -> bool {
    s.trim() == "1"
}

#[cfg(target_os = "linux")]
pub(crate) fn forwarding() -> bool {
    match std::fs::read_to_string("/proc/sys/net/ipv4/ip_forward") {
        Ok(text) => forwarding_from_text(&text),
        Err(_) => false,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn forwarding() -> bool {
    let mut value: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    let name = c"net.inet.ip.forwarding";
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&mut value as *mut libc::c_int).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    rc == 0 && value != 0
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn forwarding() -> bool {
    false
}

#[cfg(test)]
#[path = "iface_os_tests.rs"]
mod tests;
