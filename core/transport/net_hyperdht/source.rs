//! `HyperDhtDiscovery`: peer discovery by the key roster in a DHT record, as `impl Discovery`.
//! What it does: with `me` each round merges its own key into the record and reads the others, without `me` it only
//! reads; it hands out `PeerAddr::new(id)` of each new key (an address without hints — dialed by key).
//! `publish` writes once with a read-back check. What it does not know: how the store (`RecordStore`) works.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;

use super::keys::RosterKeys;
use super::pace::Pace;
use super::roster;
use super::store::RecordStore;
use super::tally::Tally;
use crate::members::network_key::SubKeys;
use crate::transport::net::{BoxFuture, Discovery, NetError, NetResult, PeerAddr, PeerId};
use crate::transport::net_mainline::record::Roster;

/// How many "write and verify by reading" rounds a one-off publish makes.
const PUBLISH_TRIES: u32 = 3;

/// Round outcome for the node: the record was read (and how many members it has), our own record was updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DhtRoundOutcome {
    pub read: bool,
    pub members: usize,
    pub published: bool,
}

/// Round outcome handler (node telemetry and log).
pub type RoundOutcomeFn = Arc<dyn Fn(DhtRoundOutcome) + Send + Sync>;

pub struct HyperDhtDiscovery {
    store: Arc<dyn RecordStore>,
    keys: RosterKeys,
    me: Option<PeerId>,
    pace: Pace,
    round_outcome: Option<RoundOutcomeFn>,
}

impl HyperDhtDiscovery {
    /// A source with the working pace; without `me` the source only reads.
    pub fn new(store: Arc<dyn RecordStore>, keys: RosterKeys, me: Option<PeerId>) -> Self {
        Self {
            store,
            keys,
            me,
            pace: Pace::production(),
            round_outcome: None,
        }
    }

    pub fn with_pace(mut self, pace: Pace) -> Self {
        self.pace = pace;
        self
    }

    /// Each round reports outward whether the record was read.
    pub fn with_round_outcome(mut self, f: RoundOutcomeFn) -> Self {
        self.round_outcome = Some(f);
        self
    }

    /// One round: the publisher writes and returns the written roster, the reader reads (no record — empty).
    async fn round(&self) -> NetResult<Roster> {
        let Some(me) = self.me else {
            let got = roster::read(&*self.store, &self.keys).await?;
            return Ok(got.map_or_else(empty_roster, |g| g.0));
        };
        let (unix, ms) = now_unix_secs_and_ms();
        roster::publish_round(&*self.store, &self.keys, me, unix, ms).await
    }

    async fn publish_checked(&self, id: PeerId) -> NetResult<()> {
        for _ in 0..PUBLISH_TRIES {
            let (unix, ms) = now_unix_secs_and_ms();
            roster::publish_round(&*self.store, &self.keys, id, unix, ms).await?;
            if self.holds(id).await? {
                return Ok(());
            }
        }
        Err(NetError::Other(anyhow::anyhow!(
            "hyperdht: record did not stick in {PUBLISH_TRIES} attempts"
        )))
    }

    async fn holds(&self, id: PeerId) -> NetResult<bool> {
        let got = roster::read(&*self.store, &self.keys).await?;
        Ok(got.is_some_and(|(r, _)| r.entries.iter().any(|e| e.id == *id.as_bytes())))
    }

    /// Hands out new keys; `false` — the receiver is closed.
    async fn report(&self, out: &mpsc::Sender<PeerAddr>, new: Vec<PeerId>) -> bool {
        for id in new {
            if out.send(PeerAddr::new(id)).await.is_err() {
                return false;
            }
        }
        true
    }

    fn spread(&self, round: u32) -> u64 {
        let base = self.me.map_or(0, |m| {
            u64::from_le_bytes(m.as_bytes()[..8].try_into().unwrap_or([0; 8]))
        });
        base.wrapping_add(u64::from(round).wrapping_mul(0x9E37_79B9_7F4A_7C15)) >> 8
    }
}

impl Discovery for HyperDhtDiscovery {
    fn name(&self) -> &'static str {
        "hyperdht"
    }

    fn publishes(&self) -> bool {
        self.me.is_some()
    }

    fn publish(&self, own_addr: &PeerAddr) -> BoxFuture<'_, NetResult<()>> {
        let id = own_addr.id;
        Box::pin(async move {
            let limit = self.pace.call_limit;
            let done = tokio::time::timeout(limit, self.publish_checked(id)).await;
            done.unwrap_or(Err(NetError::Timeout))
        })
    }

    fn run_source(self: Arc<Self>, out: mpsc::Sender<PeerAddr>) -> BoxFuture<'static, ()> {
        Box::pin(async move { self.run_rounds(out).await })
    }
}

impl HyperDhtDiscovery {
    async fn run_rounds(&self, out: mpsc::Sender<PeerAddr>) {
        let mut tally = Tally::new(Instant::now(), self.pace.summary_every);
        let (mut known, mut round, mut failures) = (HashSet::new(), 0u32, 0u32);
        while !out.is_closed() {
            let res = tokio::time::timeout(self.pace.call_limit, self.round()).await;
            let res = res.unwrap_or(Err(NetError::Timeout));
            if let Some(ids) = self.account(res, &mut tally, &mut failures) {
                let new = ids
                    .iter()
                    .filter(|i| !known.contains(*i))
                    .copied()
                    .collect();
                if !self.report(&out, new).await {
                    return;
                }
                known = ids.into_iter().collect();
            }
            tally.log_summary_if_due(Instant::now());
            tokio::time::sleep(self.pace.pause(round, failures, self.spread(round))).await;
            round = round.wrapping_add(1);
        }
    }

    /// Counts the round outcome in the log and the error counter; returns the other keys of the round (an error — `None`).
    fn account(
        &self,
        res: NetResult<Roster>,
        tally: &mut Tally,
        failures: &mut u32,
    ) -> Option<Vec<PeerId>> {
        self.count_round(&res, tally, failures);
        res.ok().map(|r| self.others_of(&r))
    }

    fn count_round(&self, res: &NetResult<Roster>, tally: &mut Tally, failures: &mut u32) {
        let now = Instant::now();
        match res {
            Ok(r) => {
                self.notify(DhtRoundOutcome {
                    read: self.me.is_some() || !r.entries.is_empty(),
                    members: r.entries.len(),
                    published: self.me.is_some(),
                });
                *failures = 0;
                tally.record_round_ok(r.entries.len(), now);
            }
            Err(e) => {
                self.notify(DhtRoundOutcome {
                    read: false,
                    members: 0,
                    published: false,
                });
                *failures += 1;
                tally.record_round_failure(e, now);
            }
        }
    }

    fn notify(&self, outcome: DhtRoundOutcome) {
        if let Some(f) = &self.round_outcome {
            f(outcome);
        }
    }

    fn others_of(&self, r: &Roster) -> Vec<PeerId> {
        let mine = self.me.map(|m| *m.as_bytes());
        let ids = r.entries.iter().filter(|e| Some(e.id) != mine);
        ids.filter_map(|e| PeerId::from_bytes(&e.id).ok()).collect()
    }
}

fn empty_roster() -> Roster {
    Roster {
        version: crate::transport::net_mainline::record::RECORD_VERSION_KEYS,
        entries: Vec::new(),
    }
}

fn now_unix_secs_and_ms() -> (u64, u64) {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    (ms / 1000, ms)
}

/// Node source: publishes its own key and reads the others through the transport's record store.
pub fn node_source(
    subkeys: &SubKeys,
    me: PeerId,
    store: Arc<dyn RecordStore>,
    round_outcome: Option<RoundOutcomeFn>,
) -> Arc<dyn Discovery> {
    let source = HyperDhtDiscovery::new(store, RosterKeys::from_subkeys(subkeys), Some(me));
    Arc::new(match round_outcome {
        Some(f) => source.with_round_outcome(f),
        None => source,
    })
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
