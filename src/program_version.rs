//! The program version of a network member — a separate key of the shared document, with its own author.
//!
//! What it does: writes the version of its own build under the key `node/version/` and reads the peers' versions through
//! the shared helpers `node_state::{read_values, own_value}`; a missing entry, a tombstone and
//! an invalid value are indistinguishable to the reader — "version unknown".
//! What it doesn't do: doesn't compare versions (comparing is the page's business) and doesn't re-check an entry
//! by time: nobody but the node itself will overwrite an entry under its own author.
//! Links: `node::start::serving::setup_serving` calls `spawn_program_version_announce`,
//! `node_ctl::snapshot::collect` calls `read_program_versions` and `current_program_version`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::net::{NetError, PeerId, Replica};
use anyhow::Result;
use bytes::Bytes;
use tokio::time::sleep;

use crate::domains::DomainList;
use crate::node_state::{own_value, read_values};
use crate::output::say;

/// The program version key in the network's shared document.
pub const PROGRAM_VERSION_KEY: &[u8] = b"node/version/";

/// The maximum length of a valid version string.
pub const MAX_PROGRAM_VERSION_LEN: usize = 32;

/// The pause before a retry if a write or a read failed.
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// The version of this build.
pub fn current_program_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// A valid version string: 1..=32 ASCII characters, the first is a digit, the rest are `[0-9A-Za-z.+-]`;
/// anything else is `None`. Never panics on any bytes.
pub fn parse_program_version(bytes: &[u8]) -> Option<String> {
    let first_is_digit = bytes.first().is_some_and(u8::is_ascii_digit);
    let fits = !bytes.is_empty() && bytes.len() <= MAX_PROGRAM_VERSION_LEN;
    let alphabet_ok = bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-'));
    let good = first_is_digit && fits && alphabet_ok;
    good.then(|| String::from_utf8_lossy(bytes).into_owned())
}

/// Writes this node's version as its own entry.
pub async fn announce_program_version(r: &dyn Replica, version: &str) -> Result<()> {
    r.set_own(
        PROGRAM_VERSION_KEY,
        Bytes::copy_from_slice(version.as_bytes()),
    )
    .await
    .map_err(NetError::into_anyhow)
}

/// Its own written version; no entry, a tombstone or an invalid value gives `Ok(None)`.
pub async fn own_program_version(r: &dyn Replica) -> Result<Option<String>> {
    let own = own_value(r, PROGRAM_VERSION_KEY).await?;
    Ok(own.and_then(|(bytes, _)| parse_program_version(&bytes)))
}

/// Writes the version only if its own entry differs: `Ok(true)` — written, `Ok(false)` — it already
/// matched, the entry is untouched.
pub async fn ensure_program_version(r: &dyn Replica, version: &str) -> Result<bool> {
    let own = own_program_version(r).await?;
    let same = own.as_deref() == Some(version);
    if same {
        return Ok(false);
    }
    announce_program_version(r, version).await?;
    Ok(true)
}

/// The version of each author whose value is valid; the others don't appear in the map.
pub async fn read_program_versions(r: &dyn Replica) -> Result<HashMap<PeerId, String>> {
    let values = read_values(r, PROGRAM_VERSION_KEY).await?;
    Ok(values
        .into_iter()
        .filter_map(|(id, bytes)| parse_program_version(&bytes).map(|v| (id, v)))
        .collect())
}

/// The outcome of one attempt: `true` — the task is finished; the log reports only a write and an error.
fn report_attempt(attempt: Result<bool>) -> bool {
    match attempt {
        Ok(written) => {
            written.then(|| {
                say(
                    "docs",
                    format!(
                        "announced program version {} to peers",
                        current_program_version()
                    ),
                )
            });
            true
        }
        Err(e) => {
            say("docs", format!("program version not written: {e:#}"));
            false
        }
    }
}

/// Once per launch brings its own entry to the build version; on an error retries in 30 s.
pub async fn run_program_version_announce(r: Arc<dyn Replica>) {
    loop {
        let attempt = ensure_program_version(r.as_ref(), current_program_version()).await;
        if report_attempt(attempt) {
            return;
        }
        sleep(RETRY_AFTER).await;
    }
}

/// Starts the version announcement as a background task; the handle goes into `boot.tasks`.
pub fn spawn_program_version_announce(list: &DomainList) -> tokio::task::AbortHandle {
    tokio::spawn(run_program_version_announce(list.shared_replica())).abort_handle()
}

#[cfg(test)]
#[path = "program_version_tests.rs"]
mod tests;
