//! Checks through the exits of members: which exits are in play now, one check through an exit, all of them at
//! once, and the results kept in memory (`Checks::exit_results`). A result is not published to the shared
//! document and not written to disk: an offline exit keeps its last result until this node restarts.

use std::collections::HashMap;

use tokio::task::JoinSet;

use crate::node::node_state;
use crate::node::output::{say, short_peer_id};
use crate::site_checks::reachability::{self, CheckRecord};
use crate::transport::net::task_label::spawn_in;
use crate::transport::net::PeerId;

use super::describe::describe_steps;
use super::state::Checks;
use super::target::{exit_targets, Target};

/// Results through exits: (domain, exit's id) → the latest record.
pub(super) type ExitResults = HashMap<(String, PeerId), CheckRecord>;

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

    /// Checks the domain through every current exit at once; returns how many results were stored.
    pub async fn check_via_exits(&self, domain: &str) -> usize {
        let peers = self.current_exit_targets().await;
        self.check_via_peers(domain, peers).await
    }

    async fn check_via_peers(&self, domain: &str, peers: Vec<PeerId>) -> usize {
        let mut set = JoinSet::new();
        for peer in peers {
            let (checks, domain) = (self.clone(), domain.to_string());
            spawn_in(&mut set, async move {
                checks.check_one_via_exit(&domain, peer).await
            });
        }
        let mut stored = 0;
        while let Some(done) = set.join_next().await {
            stored += usize::from(done.unwrap_or(false));
        }
        stored
    }
}
