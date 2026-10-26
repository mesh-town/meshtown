//! The process services: what exists once per process, not once per node. For now this is the
//! memory guard (the `mem-guard` thread); it outlives node restarts and network switches and reads
//! the live node through `GuardSources`.

use std::path::{Path, PathBuf};

use tokio::runtime::Handle;

use crate::config::config_file::ProcessSettings;
use crate::exit::{rss_ceiling_kib, ExitLimits};
use crate::node::guard::{start_memory_guard, GuardHandle, MemoryLimitsKib};
use crate::node::guard_sources::GuardSources;
use crate::node::machine_memory::{
    effective_warn_mib, for_ram, machine_ram_kib, memory_line, rss_guard_warn_mib, MemoryDefaults,
};
use crate::node::output::say;

/// The process memory guard and its summary sources.
pub struct ProcessServices {
    guard: Option<GuardHandle>,
    sources: GuardSources,
}

impl ProcessServices {
    /// Starts the services; call it inside the tokio runtime. Both memory thresholds `0` — no guard thread.
    /// `dump_dir` is where heap profile dumps go.
    pub fn start(process: &ProcessSettings, dump_dir: PathBuf) -> ProcessServices {
        let sources = GuardSources::default();
        let ram = machine_ram_kib();
        let defaults = for_ram(ram);
        let warn = effective_warn_mib(process.node_memory_warn_mib, defaults);
        let limits = MemoryLimitsKib::from_mib(warn, process.node_memory_exit_mib);
        configure_exit_limits(process, defaults, ram, warn);
        let guard = start_memory_guard(limits, sources.clone(), Handle::current(), dump_dir);
        ProcessServices { guard, sources }
    }

    /// Stops the guard thread; the wait for its next tick is interrupted at once.
    pub fn stop(self) {
        self.guard.iter().for_each(GuardHandle::stop);
    }

    /// The handle to the summary sources, for the node controller.
    pub(crate) fn sources(&self) -> GuardSources {
        self.sources.clone()
    }

    /// The guard thread was started (tests only).
    #[cfg(test)]
    pub(crate) fn guard_handle(&self) -> Option<GuardHandle> {
        self.guard.clone()
    }
}

/// Sets the process-wide exit stream limit and the RSS ceiling, and logs the memory defaults in force.
fn configure_exit_limits(
    process: &ProcessSettings,
    defaults: MemoryDefaults,
    ram: Option<u64>,
    warn: u64,
) {
    let guard_warn = rss_guard_warn_mib(process.node_memory_warn_mib, defaults);
    let limits = ExitLimits::process();
    limits.configure(
        process.exit_max_streams,
        defaults.max_streams,
        rss_ceiling_kib(guard_warn),
    );
    say("node", memory_line(ram, limits.max_streams(), warn));
}

/// The directory for heap profile dumps: next to the settings file, otherwise the current directory.
pub(crate) fn heap_dump_dir(settings_file: Option<&Path>) -> PathBuf {
    settings_file
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
