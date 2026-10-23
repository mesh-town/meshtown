//! The background check of the node's country: the schedule, the "network is present" requirement and the query loop itself.
//!
//! What it does: at start waits out the rest of `COUNTRY_RECHECK_AFTER` of its own entry (if it is fresh, there is no point
//! hitting the network at once), then waits for the first network report for up to 30 s (its absence in the first seconds
//! after start is not "no network"), then in a cycle — the network is present and a source answered → a write to the
//! document and a pause of `COUNTRY_RECHECK_AFTER`; the network is present but all three sources refused → a pause of `FAIL_RETRY`; no
//! network → a pause of `OFFLINE_RETRY`.
//! What it doesn't do: doesn't decide the sources themselves and parsing the answer — that is `geoip::fetch`/`sources`; doesn't
//! decide the format and validity of the country code — `geoip::country`; doesn't erase the last known
//! country on failure — it just doesn't call `announce_country`.
//! Links: `node::start::serving::setup_serving` calls `spawn_country_check` and puts the handle into
//! `boot.tasks`; errors go only to the log (tag `geo`) — the loop doesn't stop the node.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::node::output::say;
use crate::proxy::domains::DomainList;
use crate::transport::net::{NetInfo, Replica, Transport};

use super::board::{announce_country, own_country_record};
use super::fetch::lookup_country;

#[cfg(test)]
#[path = "task_tests.rs"]
mod tests;

/// How often to recheck the country after a successful determination.
const COUNTRY_RECHECK_AFTER: Duration = Duration::from_secs(2 * 60 * 60);
/// The pause after all three sources failed.
const FAIL_RETRY: Duration = Duration::from_secs(10 * 60);
/// The pause when the network is not there yet or no longer.
const OFFLINE_RETRY: Duration = Duration::from_secs(60);

/// The outcome of one check round — decides how long to wait until the next.
pub enum Outcome {
    Found,
    Failed,
    Offline,
}

/// The pause until the next round by the outcome of this one — the only `match` on `Outcome`.
pub fn pause_after_outcome(outcome: Outcome) -> Duration {
    match outcome {
        Outcome::Found => COUNTRY_RECHECK_AFTER,
        Outcome::Failed => FAIL_RETRY,
        Outcome::Offline => OFFLINE_RETRY,
    }
}

/// How long to wait before the first check after start, looking at the time of its own entry: no entry,
/// it is not younger than `COUNTRY_RECHECK_AFTER` or its time is in the future (the node's clock is behind) — zero, otherwise the rest until
/// `COUNTRY_RECHECK_AFTER`.
pub fn wait_before_check(written_at_us: Option<u64>, now_us: u64) -> Duration {
    let Some(at) = written_at_us else {
        return Duration::ZERO;
    };
    if at > now_us {
        return Duration::ZERO;
    }
    COUNTRY_RECHECK_AFTER.saturating_sub(Duration::from_micros(now_us - at))
}

/// The current time in microseconds since the Unix epoch — the same scale as `ReplicaEntry::timestamp()`.
fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

/// No network: the first network report came and said "not seen".
pub(super) fn net_absent(info: &NetInfo) -> bool {
    info.net_seen == Some(false)
}

/// Keeps this node's country fresh. Doesn't finish on its own — it is aborted together with the
/// node's other background tasks via `AbortHandle` (see `spawn_country_check`).
pub async fn run_country_check(transport: Arc<dyn Transport>, r: Arc<dyn Replica>) {
    wait_out_fresh_own_country(r.as_ref()).await;

    transport.wait_net_info().await;

    let mut was_offline = false;
    loop {
        if net_absent(&transport.net_info()) {
            if !was_offline {
                say("geo", "no network, lookup postponed");
                was_offline = true;
            }
            tokio::time::sleep(pause_after_outcome(Outcome::Offline)).await;
            continue;
        }
        was_offline = false;
        run_one_check(r.as_ref()).await;
    }
}

/// Waits out the rest of `COUNTRY_RECHECK_AFTER` of its already written country, if it is still fresh.
async fn wait_out_fresh_own_country(replica: &dyn Replica) {
    let now_us = now_micros();
    let Some((country, at)) = own_country_record(replica).await.ok().flatten() else {
        return;
    };
    let wait = wait_before_check(Some(at), now_us);
    if wait.is_zero() {
        return;
    }
    say(
        "geo",
        format!(
            "country {} recorded {} min ago, next check in {} min",
            country.as_str(),
            now_us.saturating_sub(at) / 60_000_000,
            wait.as_secs() / 60,
        ),
    );
    tokio::time::sleep(wait).await;
}

/// One round: asks the sources, on success writes the country, waits `COUNTRY_RECHECK_AFTER`/`FAIL_RETRY` by
/// the outcome. Write errors go only to the log, the country stays found.
async fn run_one_check(r: &dyn Replica) {
    match lookup_country().await {
        Ok((country, source)) => {
            let outcome = match announce_country(r, country).await {
                Ok(()) => {
                    say(
                        "geo",
                        format!("country {} from {}", country.as_str(), source.spec().host),
                    );
                    Outcome::Found
                }
                Err(e) => {
                    say(
                        "geo",
                        format!("country {} not recorded: {e:#}", country.as_str()),
                    );
                    Outcome::Failed
                }
            };
            tokio::time::sleep(pause_after_outcome(outcome)).await;
        }
        Err(reasons) => {
            say("geo", format!("country not determined: {reasons}"));
            tokio::time::sleep(pause_after_outcome(Outcome::Failed)).await;
        }
    }
}

/// Starts the background country lookup as a separate task.
pub fn spawn_country_check(
    transport: Arc<dyn Transport>,
    list: &DomainList,
) -> tokio::task::AbortHandle {
    crate::node::output::spawn(run_country_check(transport, list.shared_replica())).abort_handle()
}
