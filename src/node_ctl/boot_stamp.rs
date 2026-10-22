//! Stamp of one lap of a slot — `boot_id` for the page and tray, and the `uptime_sec` counter.

/// Stamp of one node lap: `boot_id` (8 hex characters) and the moment the lap began.
pub(crate) struct BootStamp {
    id: String,
    since: std::time::Instant,
}

impl BootStamp {
    pub(crate) fn new() -> Self {
        Self {
            id: gen_boot_id(),
            since: std::time::Instant::now(),
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn uptime_sec(&self) -> u64 {
        self.since.elapsed().as_secs()
    }

    #[cfg(test)]
    pub(crate) fn backdated(secs: u64) -> Self {
        let now = std::time::Instant::now();
        let since = now
            .checked_sub(std::time::Duration::from_secs(secs))
            .unwrap_or(now);
        Self {
            id: gen_boot_id(),
            since,
        }
    }
}

/// Eight random hex characters from four random bytes for `boot_id`. The randomness source is not
/// security-critical (this is a displayed launch identifier, not a key) — a generator failure
/// does not panic but yields zero bytes.
fn gen_boot_id() -> String {
    let mut bytes = [0u8; 4];
    let _ = crate::network_key::fill_random(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "boot_stamp_tests.rs"]
mod tests;
