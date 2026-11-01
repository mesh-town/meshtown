//! Node discovery sources from ready-made lists: `invite` (addresses from the invite) and `cache`
//! (addresses from `peers-cache.json`). They hand out the list once and wait for the receiver to close.
//! `node_sources` builds the node's list of sources from the config (no I/O).

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::members::network_key::SubKeys;
use crate::node::config::NodeConfig;
use crate::node::output::{say, short_peer_id};
use crate::node::telemetry::Telemetry;
use crate::transport::net::stream::BoxFuture;
use crate::transport::net::{Discovery, NetResult, PeerAddr, PeerId};
use crate::transport::net_hyperdht::{DhtRoundOutcome, RecordStore, RoundOutcomeFn};
use crate::transport::peers_cache::PeersCache;

/// Node discovery sources in the order of names from `cfg.settings.net_choice.discovery`: each only if it
/// has input (an invite, a non-empty cache, `hyper` DHT records); a name without a source is skipped.
pub(crate) fn node_sources(
    cfg: &NodeConfig,
    cache: &PeersCache,
    me: PeerId,
    subkeys: &SubKeys,
    tel: &Arc<Telemetry>,
    records: Option<Arc<dyn RecordStore>>,
) -> Vec<Arc<dyn Discovery>> {
    let from = |name: &String| source_named(name, cfg, cache, me, (subkeys, tel), &records);
    cfg.settings
        .net_choice
        .discovery
        .iter()
        .filter_map(from)
        .collect()
}

/// The source by config name; `None` — the name isn't in the node or it has no input.
fn source_named(
    name: &str,
    cfg: &NodeConfig,
    cache: &PeersCache,
    me: PeerId,
    (subkeys, tel): (&SubKeys, &Arc<Telemetry>),
    records: &Option<Arc<dyn RecordStore>>,
) -> Option<Arc<dyn Discovery>> {
    match name {
        "invite" if !cfg.initial_peers.is_empty() => {
            Some(Arc::new(InviteSource::new(cfg.initial_peers.clone(), me)))
        }
        "cache" if !cache.is_empty() => Some(Arc::new(PeersCacheSource::new(cache.addrs(), me))),
        "hyperdht" => hyperdht_source(subkeys, me, records, tel),
        _ => None,
    }
}

/// The `hyperdht` source lives on the DHT records of the `hyper` transport; without them it is skipped. The result of each
/// round goes to telemetry (`seen_handler`).
fn hyperdht_source(
    subkeys: &SubKeys,
    me: PeerId,
    records: &Option<Arc<dyn RecordStore>>,
    tel: &Arc<Telemetry>,
) -> Option<Arc<dyn Discovery>> {
    let Some(store) = records else {
        say("hyperdht", "hyperdht without the hyper transport — skipped");
        return None;
    };
    Some(crate::transport::net_hyperdht::node_source(
        subkeys,
        me,
        store.clone(),
        Some(seen_handler(tel.clone())),
    ))
}

/// Rounds in a row without DHT nodes and other members after which the line is written (and every such number again).
const EMPTY_ROUNDS_LINE_EVERY: u32 = 10;

/// What the handler remembers between rounds.
#[derive(Default)]
struct RoundMemory {
    members: Option<usize>,
    table: u64,
    empty_rounds: u32,
}

impl RoundMemory {
    /// The table size of the round (`None` keeps the previous value).
    fn table_of(&mut self, s: &DhtRoundOutcome) -> u64 {
        if let Some(n) = s.table {
            self.table = n as u64;
        }
        self.table
    }

    /// Counts a round that is joined, has no other members and no DHT nodes; returns the streak length
    /// when the line is due.
    fn empty_streak_due(&mut self, s: &DhtRoundOutcome, joined: bool) -> Option<u32> {
        if !(joined && s.others == 0 && self.table == 0) {
            self.empty_rounds = 0;
            return None;
        }
        self.empty_rounds += 1;
        (self.empty_rounds % EMPTY_ROUNDS_LINE_EVERY == 0).then_some(self.empty_rounds)
    }
}

/// Handler of a `hyperdht` round result: telemetry (read, publish, directory join, routing table size) and log
/// lines: `[hyperdht] record read from DHT: peers N` — on the first success and when N changes, and
/// `[hyperdht] no DHT nodes and no other members for N rounds` — every 10th round of such a streak.
pub(crate) fn seen_handler(tel: Arc<Telemetry>) -> RoundOutcomeFn {
    let memory = Mutex::new(RoundMemory::default());
    Arc::new(move |s: DhtRoundOutcome| {
        let mut mem = memory.lock().unwrap_or_else(|e| e.into_inner());
        if s.read {
            tel.set_dht_read(s.members);
            if mem.members != Some(s.members) {
                mem.members = Some(s.members);
                say(
                    "hyperdht",
                    format!("record read from DHT: peers {}", s.members),
                );
            }
        }
        if s.published {
            tel.note_publish();
        }
        let joined = s.read || s.published;
        let nodes = mem.table_of(&s);
        tel.set_dht_joined_and_size(joined, nodes);
        if let Some(n) = mem.empty_streak_due(&s, joined) {
            say(
                "hyperdht",
                format!("no DHT nodes and no other members for {n} rounds"),
            );
        }
    })
}

/// Source labels: the log tag and the text of the summary line (the accepted count is substituted).
struct Lines {
    tag: &'static str,
    total: fn(usize) -> String,
}

/// Hands the list out to `out`: our own id is skipped, a `try_send` refusal isn't retried. An empty
/// list — no line. Then waits for `out` to close.
async fn hand_over(list: Vec<PeerAddr>, me: PeerId, out: mpsc::Sender<PeerAddr>, lines: Lines) {
    if !list.is_empty() {
        let taken = send_addrs_except_own(&list, me, &out, lines.tag);
        say(lines.tag, (lines.total)(taken));
    }
    out.closed().await;
}

fn send_addrs_except_own(
    list: &[PeerAddr],
    me: PeerId,
    out: &mpsc::Sender<PeerAddr>,
    tag: &str,
) -> usize {
    let mut taken = 0usize;
    for addr in list.iter().filter(|a| a.id != me) {
        match out.try_send(addr.clone()) {
            Ok(()) => taken += 1,
            Err(e) => say(
                tag,
                format!("address {} not accepted: {e}", short_peer_id(&addr.id)),
            ),
        }
    }
    taken
}

/// The `cache` source: member addresses from the peer cache.
pub(crate) struct PeersCacheSource {
    peers: Vec<PeerAddr>,
    me: PeerId,
}

impl PeersCacheSource {
    pub(crate) fn new(peers: Vec<PeerAddr>, me: PeerId) -> Self {
        Self { peers, me }
    }
}

impl Discovery for PeersCacheSource {
    fn name(&self) -> &'static str {
        "cache"
    }

    fn publish(&self, _own_addr: &PeerAddr) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn run_source(self: Arc<Self>, out: mpsc::Sender<PeerAddr>) -> BoxFuture<'static, ()> {
        let lines = Lines {
            tag: "cache",
            total: |n| format!("peer addresses taken from cache: {n}"),
        };
        Box::pin(hand_over(self.peers.clone(), self.me, out, lines))
    }
}

/// The `invite` source: member addresses from the invite (`--invite`).
pub(crate) struct InviteSource {
    peers: Vec<PeerAddr>,
    me: PeerId,
}

impl InviteSource {
    pub(crate) fn new(peers: Vec<PeerAddr>, me: PeerId) -> Self {
        Self { peers, me }
    }
}

impl Discovery for InviteSource {
    fn name(&self) -> &'static str {
        "invite"
    }

    fn publish(&self, _own_addr: &PeerAddr) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn run_source(self: Arc<Self>, out: mpsc::Sender<PeerAddr>) -> BoxFuture<'static, ()> {
        let lines = Lines {
            tag: "invite",
            total: |n| format!("addresses taken from invite: {n}"),
        };
        Box::pin(hand_over(self.peers.clone(), self.me, out, lines))
    }
}

#[cfg(test)]
#[path = "discovery_sources_tests.rs"]
pub(in crate::node) mod tests;
