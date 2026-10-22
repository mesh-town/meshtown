//! The process services: what exists once per process, not once per node. For now this is the
//! memory guard (the `mem-guard` thread); it outlives node restarts and network switches and reads
//! the live node through `GuardSources`.

use std::path::{Path, PathBuf};

use tokio::runtime::Handle;

use crate::config::config_file::ProcessSettings;
use crate::members::membership::Members;
use crate::node::guard::{start_memory_guard, GuardHandle, GuardSources, MemoryLimitsKib};
use crate::site_checks::checks::Checks;

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
        let limits =
            MemoryLimitsKib::from_mib(process.node_memory_warn_mib, process.node_memory_exit_mib);
        let guard = start_memory_guard(limits, sources.clone(), Handle::current(), dump_dir);
        ProcessServices { guard, sources }
    }

    /// Stops the guard thread; the wait for its next tick is interrupted at once.
    pub fn stop(self) {
        self.guard.iter().for_each(GuardHandle::stop);
    }

    /// The live node's handles for the guard summary.
    pub fn register(&self, members: Members, checks: Checks) {
        self.sources.set(members, checks);
    }

    /// The node is gone: the summary reads nothing.
    pub fn unregister(&self) {
        self.sources.clear();
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

/// The directory for heap profile dumps: next to the settings file, otherwise the current directory.
pub(crate) fn heap_dump_dir(settings_file: Option<&Path>) -> PathBuf {
    settings_file
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
