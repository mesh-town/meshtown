//! Live state of a node (relay, NAT/UDP, DHT, RSS, live members) in one place.
//!
//! What it does: keeps a copy of the current state under a `std::sync::Mutex`, written by the node's
//! tasks (`connection_report`, `dht`, `node::run_node_loop`) and read as a snapshot by the node page
//! (`node_ctl/snapshot.rs`) via `view()`.
//! What it does not do: it does not wait on `.await` and does not print — setters only assign fields.
//! Links: `node::start_node` creates `Telemetry::new()` and hands out `Arc<Telemetry>` to the writers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::transport::net::PeerId;

use crate::web_gui::PathKindView;

/// Live record of one network member.
#[derive(Clone, Debug)]
pub struct PeerLive {
    pub online: bool,
    pub path: Option<PathKindView>,
    pub rtt_ms: Option<u64>,
    pub last_seen: Instant,
}

impl PeerLive {
    fn blank() -> PeerLive {
        PeerLive {
            online: false,
            path: None,
            rtt_ms: None,
            last_seen: Instant::now(),
        }
    }
}

/// Copy of the node's live state — what the page snapshot reads.
#[derive(Clone, Debug, Default)]
pub struct TelemetryView {
    pub dht_joined: bool,
    pub dht_nodes: u64,
    pub last_publish: Option<Instant>,
    pub record_published: bool,
    /// When the record was last read from HyperDHT, and how many members it holds.
    pub dht_read: Option<(Instant, usize)>,
    pub peers: HashMap<PeerId, PeerLive>,
}

/// Store of the node's live state behind a leaf lock.
pub struct Telemetry(Mutex<TelemetryView>);

impl Telemetry {
    pub fn new() -> Arc<Telemetry> {
        Arc::new(Telemetry(Mutex::new(TelemetryView::default())))
    }

    /// Copy of the whole current state.
    pub fn view(&self) -> TelemetryView {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn with_mut(&self, f: impl FnOnce(&mut TelemetryView)) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard);
    }

    /// The record was read from the DHT just now: it holds `members` members.
    pub(crate) fn set_dht_read(&self, members: usize) {
        self.with_mut(|v| v.dht_read = Some((Instant::now(), members)));
    }

    // DHT counters are called by the node's `DhtRoundOutcome` handler.
    pub(crate) fn set_dht_joined_and_size(&self, joined: bool, nodes: u64) {
        self.with_mut(|v| {
            v.dht_joined = joined;
            v.dht_nodes = nodes;
        });
    }

    pub(crate) fn note_publish(&self) {
        self.with_mut(|v| {
            v.last_publish = Some(Instant::now());
            v.record_published = true;
        });
    }

    pub(crate) fn peer_up(&self, id: PeerId, path: Option<PathKindView>, rtt_ms: Option<u64>) {
        self.with_mut(|v| {
            v.peers.insert(
                id,
                PeerLive {
                    online: true,
                    path,
                    rtt_ms,
                    last_seen: Instant::now(),
                },
            );
        });
    }

    pub(crate) fn peer_down(&self, id: PeerId) {
        self.with_mut(|v| {
            let entry = v.peers.entry(id).or_insert_with(PeerLive::blank);
            entry.online = false;
            entry.last_seen = Instant::now();
        });
    }

    pub(crate) fn record_peer_path(
        &self,
        id: PeerId,
        path: Option<PathKindView>,
        rtt_ms: Option<u64>,
    ) {
        self.with_mut(|v| {
            let entry = v.peers.entry(id).or_insert_with(PeerLive::blank);
            entry.path = path;
            entry.rtt_ms = rtt_ms;
            entry.last_seen = Instant::now();
        });
    }
}

/// Current and peak RSS of the process in KiB; `0` in either means the data is unavailable.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn rss_kib() -> (u64, u64) {
    let Ok(content) = std::fs::read_to_string("/proc/self/status") else {
        return (0, 0);
    };
    let (mut cur, mut peak) = (0u64, 0u64);
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            cur = digits_kib(rest);
        } else if let Some(rest) = line.strip_prefix("VmHWM:") {
            peak = digits_kib(rest);
        }
    }
    (cur, peak)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn digits_kib(s: &str) -> u64 {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or(0)
}

#[cfg(target_os = "macos")]
pub fn rss_kib() -> (u64, u64) {
    let cur = current_bytes().map(|b| b / 1024).unwrap_or(0);
    let peak = peak_bytes().map(|b| b / 1024).unwrap_or(0);
    (cur, peak)
}

/// `proc_pidinfo(getpid(), PROC_PIDTASKINFO, ..)` — current RSS of the own process, in bytes.
#[cfg(target_os = "macos")]
fn current_bytes() -> Option<u64> {
    unsafe {
        let mut info: libc::proc_taskinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
        let ret = libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTASKINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        );
        if ret == size {
            Some(info.pti_resident_size)
        } else {
            None
        }
    }
}

/// `getrusage(RUSAGE_SELF, ..)` — peak RSS of the own process; on Darwin `ru_maxrss` is already in
/// bytes (not KiB as on Linux — the only reason for dividing by 1024 here).
#[cfg(target_os = "macos")]
fn peak_bytes() -> Option<u64> {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        let ret = libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        if ret == 0 {
            Some(usage.ru_maxrss as u64)
        } else {
            None
        }
    }
}

/// `GetProcessMemoryInfo` on the own process: working set (current) and peak working set, in KiB.
#[cfg(windows)]
pub fn rss_kib() -> (u64, u64) {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        c.cb = size;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut c, size) == 0 {
            return (0, 0);
        }
        (
            c.WorkingSetSize as u64 / 1024,
            c.PeakWorkingSetSize as u64 / 1024,
        )
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows
)))]
pub fn rss_kib() -> (u64, u64) {
    (0, 0)
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
