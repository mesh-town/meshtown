//! The tokio runtime of the process entry.

use std::future::Future;
use std::time::Duration;

/// How long the exit waits for blocking threads (a stuck DNS lookup, a hung disk) before leaving them behind.
pub(crate) const EXIT_DRAIN: Duration = Duration::from_secs(1);

/// Runs `body` on a multi-thread runtime and returns its output; blocking threads still running after the body
/// are given `EXIT_DRAIN` and then abandoned, so they cannot hold the exit of the process.
pub(crate) fn run_to_exit<T>(body: impl Future<Output = T>) -> T {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime");
    let out = rt.block_on(body);
    rt.shutdown_timeout(EXIT_DRAIN);
    out
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
