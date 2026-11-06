//! Checks through the exits of members: which exits are in play now, one check through an exit, all of them at
//! once, and the results kept in memory (`Checks::exit_results`). A result is not published to the shared
//! document and not written to disk: an offline exit keeps its last result until this node restarts.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::node::node_state;
use crate::node::output::{say, short_peer_id};
use crate::site_checks::reachability::{self, CheckRecord};
use crate::transport::net::task_label::spawn;
use crate::transport::net::PeerId;

use super::describe::describe_steps;
use super::state::Checks;
use super::target::{exit_targets, Target};

/// Results through exits: (domain, exit's id) → the latest record.
pub(super) type ExitResults = HashMap<(String, PeerId), CheckRecord>;

/// The (domain, exit) pairs whose check through the exit is running now: a pair is not started twice.
#[derive(Default)]
pub(super) struct PairsInProgress(Mutex<HashSet<(String, PeerId)>>);

/// Holds a pair in `PairsInProgress`; dropping it (end, panic or abort of the task) frees the pair.
pub(super) struct PairGuard {
    pairs: Arc<PairsInProgress>,
    key: (String, PeerId),
}

impl PairsInProgress {
    /// Takes the pair; `None` means its previous check has not finished yet.
    pub(super) fn try_begin(self: &Arc<Self>, domain: &str, peer: PeerId) -> Option<PairGuard> {
        let key = (domain.to_string(), peer);
        let fresh = self
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone());
        fresh.then(|| PairGuard {
            pairs: Arc::clone(self),
            key,
        })
    }
}

impl Drop for PairGuard {
    fn drop(&mut self) {
        let mut set = self.pairs.0.lock().unwrap_or_else(|e| e.into_inner());
        set.remove(&self.key);
    }
}

impl Checks {
    /// Members to check through right now: exit flag on (from the shared document), online, not this node.
    pub(super) async fn current_exit_targets(&self) -> Vec<PeerId> {
        let replica = self.shared_replica.as_ref();
        let Ok(flags) = node_state::read_exit_flags(replica).await else {
            return Vec::new();
        };
        exit_targets(&flags, &self.members.ids(), replica.me())
    }

    /// The latest results through exits, a copy: (domain, exit's id) → record.
    pub fn exit_results(&self) -> ExitResults {
        self.exit_results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Checks the domain through one exit: logs, keeps the result and moves the pair's schedule.
    /// Returns whether a result was stored; an unreachable or refusing exit leaves the old result.
    pub(super) async fn check_one_via_exit(&self, domain: &str, peer: PeerId) -> bool {
        let target = Target::Exit(peer);
        let who = short_peer_id(&peer);
        let asked = reachability::check_domain_via_exit(
            &self.check_config,
            &self.stack,
            &self.pool,
            peer,
            domain,
        )
        .await;
        match asked {
            Ok(record) => {
                say(
                    "check",
                    format!("{domain} via {who}: {}", describe_steps(&record)),
                );
                let verdict = record.verdict;
                self.store_exit_result(domain, peer, record);
                self.schedule_after_verdict(domain, target, verdict).await;
                true
            }
            Err(e) => {
                say("check", format!("{domain} via {who}: check failed: {e:#}"));
                self.postpone(domain, target).await;
                false
            }
        }
    }

    fn store_exit_result(&self, domain: &str, peer: PeerId, record: CheckRecord) {
        let mut results = self.exit_results.lock().unwrap_or_else(|e| e.into_inner());
        results.insert((domain.to_string(), peer), record);
    }

    /// Starts a background check of the domain through one exit and does not wait for it; `false` means
    /// the pair is already being checked.
    pub(super) fn spawn_via_exit(&self, domain: &str, peer: PeerId) -> bool {
        let Some(guard) = self.pairs_in_progress.try_begin(domain, peer) else {
            return false;
        };
        let (checks, domain) = (self.clone(), domain.to_string());
        spawn(async move {
            let _guard = guard;
            checks.check_one_via_exit(&domain, peer).await;
        });
        true
    }

    /// Starts checks of the domain through every current exit in the background; returns how many were
    /// started (a pair whose previous check is still running is skipped). Results appear in `exit_results`.
    pub async fn check_via_exits(&self, domain: &str) -> usize {
        let peers = self.current_exit_targets().await;
        peers
            .into_iter()
            .filter(|peer| self.spawn_via_exit(domain, *peer))
            .count()
    }
}

#[cfg(test)]
#[path = "exits_tests.rs"]
mod tests;
