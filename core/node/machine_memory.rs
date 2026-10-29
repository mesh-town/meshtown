//! Memory size of the machine and the defaults that follow from it.
//!
//! What it does: reads the total RAM once, picks the exit stream cap and the memory warning level
//! for small, middle and large machines.
//! What it doesn't do: it doesn't watch the memory of the process (see `telemetry` and `guard`).

// Most callers live in the node layer; the core without it keeps only the tier table.
#![cfg_attr(not(feature = "net-hyper"), allow(dead_code))]

/// Factory default of `node.memory_warn_mib`, MiB. On a machine with ≤512 MiB this exact value means "choose by machine memory".
pub const NODE_MEMORY_WARN_MIB_DEFAULT: u64 = 400;

/// Machines up to this size (KiB) get the small defaults.
const SMALL_RAM_KIB: u64 = 131_072;
/// Machines up to this size (KiB) get the middle defaults.
const MIDDLE_RAM_KIB: u64 = 524_288;

/// Defaults for one class of machines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MemoryDefaults {
    pub max_streams: u64,
    /// Cap on unacknowledged bytes per exit stream toward the client, KiB; 0 = none.
    pub stream_window_kib: u64,
    pub warn_mib: u64,
    /// The exit refuses new streams near the warning level.
    pub rss_guard: bool,
}

impl MemoryDefaults {
    pub(crate) const SMALL: MemoryDefaults = MemoryDefaults {
        max_streams: 20,
        stream_window_kib: 128,
        warn_mib: 24,
        rss_guard: true,
    };
    pub(crate) const MIDDLE: MemoryDefaults = MemoryDefaults {
        max_streams: 50,
        stream_window_kib: 0,
        warn_mib: 64,
        rss_guard: true,
    };
    pub(crate) const LARGE: MemoryDefaults = MemoryDefaults {
        max_streams: 200,
        stream_window_kib: 0,
        warn_mib: NODE_MEMORY_WARN_MIB_DEFAULT,
        rss_guard: false,
    };
}

/// Total RAM of the machine in KiB; `None` when unknown.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn machine_ram_kib() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    mem_total_kib(&text)
}

#[cfg(target_os = "macos")]
pub(crate) fn machine_ram_kib() -> Option<u64> {
    let mut bytes: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    let name = c"hw.memsize";
    // SAFETY: `name` is NUL-terminated; `bytes` and `len` describe a valid u64 buffer.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&mut bytes as *mut u64).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (rc == 0 && bytes != 0).then_some(bytes / 1024)
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
pub(crate) fn machine_ram_kib() -> Option<u64> {
    None
}

/// The `MemTotal:` value of `/proc/meminfo` text, in KiB.
#[cfg(any(test, target_os = "linux", target_os = "android"))]
fn mem_total_kib(meminfo: &str) -> Option<u64> {
    let line = meminfo.lines().find_map(|l| l.strip_prefix("MemTotal:"))?;
    let digits = line.split_whitespace().next()?;
    digits.parse().ok()
}

pub(crate) fn for_ram(ram_kib: Option<u64>) -> MemoryDefaults {
    match ram_kib {
        Some(kib) if kib <= SMALL_RAM_KIB => MemoryDefaults::SMALL,
        Some(kib) if kib <= MIDDLE_RAM_KIB => MemoryDefaults::MIDDLE,
        _ => MemoryDefaults::LARGE,
    }
}

/// The warning level in MiB for the watchdog: the factory value follows the machine.
pub(crate) fn effective_warn_mib(stored: u64, d: MemoryDefaults) -> u64 {
    if stored == NODE_MEMORY_WARN_MIB_DEFAULT {
        return d.warn_mib;
    }
    stored
}

/// The warning level in MiB for the exit memory guard; 0 when the guard is off.
pub(crate) fn rss_guard_warn_mib(stored: u64, d: MemoryDefaults) -> u64 {
    if d.rss_guard || stored != NODE_MEMORY_WARN_MIB_DEFAULT {
        return effective_warn_mib(stored, d);
    }
    0
}

/// The start-up log line with the machine memory and the effective limits.
pub(crate) fn memory_line(ram_kib: Option<u64>, max_streams: u64, warn_mib: u64) -> String {
    let machine = match ram_kib {
        Some(kib) => format!("{} MiB", kib / 1024),
        None => "unknown".to_string(),
    };
    format!("memory: machine {machine}, exit streams {max_streams}, warn {warn_mib} MiB")
}

#[cfg(test)]
#[path = "machine_memory_tests.rs"]
mod tests;
