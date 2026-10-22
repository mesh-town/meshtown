//! Node discovery sources from ready-made lists: `invite` (addresses from the invite) and `cache`
//! (addresses from `peers-cache.json`). They hand out the list once and wait for the receiver to close.
//! `node_sources` builds the node's list of sources from the config (no I/O).

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::net::stream::BoxFuture;
use crate::net::{Discovery, NetResult, PeerAddr, PeerId};
use crate::net_hyperdht::{DhtRoundOutcome, RecordStore, RoundOutcomeFn};
use crate::network_key::SubKeys;
use crate::node::config::NodeConfig;
use crate::output::{say, short_peer_id};
use crate::peers_cache::PeersCache;
use crate::telemetry::Telemetry;

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
    cfg.settings.net_choice.discovery.iter().filter_map(from).collect()
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
    Some(crate::net_hyperdht::node_source(subkeys, me, store.clone(), Some(seen_handler(tel.clone()))))
}

/// Handler of a `hyperdht` round result: telemetry (read, publish, directory join) and a log line
/// `[hyperdht] record read from DHT: peers N` — on the first success and when N changes.
pub(crate) fn seen_handler(tel: Arc<Telemetry>) -> RoundOutcomeFn {
    let last: Mutex<Option<usize>> = Mutex::new(None);
    Arc::new(move |s: DhtRoundOutcome| {
        if s.read {
            tel.set_dht_read(s.members);
            let mut prev = last.lock().unwrap_or_else(|e| e.into_inner());
            if *prev != Some(s.members) {
                *prev = Some(s.members);
                say("hyperdht", format!("record read from DHT: peers {}", s.members));
            }
        }
        if s.published {
            tel.note_publish();
        }
        tel.set_dht_joined_and_size(s.read || s.published, 0);
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

fn send_addrs_except_own(list: &[PeerAddr], me: PeerId, out: &mpsc::Sender<PeerAddr>, tag: &str) -> usize {
    let mut taken = 0usize;
    for addr in list.iter().filter(|a| a.id != me) {
        match out.try_send(addr.clone()) {
            Ok(()) => taken += 1,
            Err(e) => say(tag, format!("address {} not accepted: {e}", short_peer_id(&addr.id))),
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
