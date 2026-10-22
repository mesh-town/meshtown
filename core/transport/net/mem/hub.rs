//! Hub of the substitute `mem` network: node slots, cuts, delay, discovery boards; `MemNet` is the hub's handle.
//! Paths are in `path`, the connector in `connector`, discovery in `discovery`.

use crate::transport::net::addr::{AddrHint, PeerAddr};
use crate::transport::net::error::{NetError, NetResult};
use crate::transport::net::id::{NodeSecret, PeerId};
use crate::transport::net::mem::connector::MemConnector;
use crate::transport::net::mem::discovery::{Board, MemDiscovery};
use crate::transport::net::mem::path::{Line, MemPath};
use crate::transport::net::mem::MEM_HINT_KIND;
use portable_atomic::AtomicU64;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, Weak};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// Incoming path queue of one slot.
const ACCEPT_QUEUE: usize = 64;
static NEXT_NET: AtomicU64 = AtomicU64::new(1);

pub(super) fn encode_hint(net: u64, slot: u64) -> AddrHint {
    let mut bytes = net.to_be_bytes().to_vec();
    bytes.extend_from_slice(&slot.to_be_bytes());
    AddrHint::Opaque {
        kind: MEM_HINT_KIND,
        bytes,
    }
}

/// `(network, slot)` from its own hint; any other hint gives `None`.
pub(super) fn decode_hint(hint: &AddrHint) -> Option<(u64, u64)> {
    let AddrHint::Opaque { kind, bytes } = hint else {
        return None;
    };
    let raw: [u8; 16] = bytes.as_slice().try_into().ok()?;
    if *kind != MEM_HINT_KIND {
        return None;
    }
    let net = u64::from_be_bytes(raw[..8].try_into().ok()?);
    let slot = u64::from_be_bytes(raw[8..].try_into().ok()?);
    Some((net, slot))
}

struct Slot {
    peer: PeerId,
    queue: mpsc::Sender<MemPath>,
    /// The "slot removed" signal: wakes a `dial` waiting for room in the target's queue.
    gone: watch::Sender<bool>,
    lines: Vec<Weak<Line>>,
}

impl Slot {
    fn close_lines(&self, slot: u64) {
        for line in self.lines.iter().filter_map(Weak::upgrade) {
            line.close_slot(slot);
        }
    }
}

#[derive(Default)]
struct HubState {
    next_slot: u64,
    slots: HashMap<u64, Slot>,
    cuts: HashSet<(PeerId, PeerId)>,
    delay: Duration,
    boards: HashMap<&'static str, Arc<Board>>,
}

fn cut_key(a: PeerId, b: PeerId) -> (PeerId, PeerId) {
    (a.min(b), a.max(b))
}

impl HubState {
    /// Removes a slot: the accept queue is closed, its lines are closed.
    fn remove(&mut self, slot: u64) {
        let Some(gone) = self.slots.remove(&slot) else {
            return;
        };
        gone.close_lines(slot);
        gone.gone.send_replace(true);
    }

    fn live_of(&self, peer: PeerId) -> Vec<u64> {
        let same = |(_, s): &(&u64, &Slot)| s.peer == peer;
        self.slots.iter().filter(same).map(|(id, _)| *id).collect()
    }

    /// The target slot by the address hints: own key — the slot, foreign key — `WrongPeer`, empty — `NoRoute`.
    fn pick_target(&self, net: u64, to: &PeerAddr) -> NetResult<u64> {
        let live: Vec<(u64, &Slot)> = to
            .hints
            .iter()
            .filter_map(decode_hint)
            .filter(|(n, _)| *n == net)
            .filter_map(|(_, s)| self.slots.get(&s).map(|slot| (s, slot)))
            .collect();
        if let Some((slot, _)) = live.iter().find(|(_, s)| s.peer == to.id) {
            return Ok(*slot);
        }
        match live.first() {
            Some((_, other)) => Err(NetError::WrongPeer {
                want: to.id,
                got: other.peer,
            }),
            None => Err(NetError::NoRoute),
        }
    }
}

/// Shared state of one `mem` network.
pub(super) struct Hub {
    pub(super) net_id: u64,
    this: Weak<Hub>,
    state: Mutex<HubState>,
}

impl Hub {
    pub(super) fn new() -> Arc<Hub> {
        let net_id = NEXT_NET.fetch_add(1, Ordering::Relaxed);
        Arc::new_cyclic(|this| Hub {
            net_id,
            this: this.clone(),
            state: Mutex::new(HubState::default()),
        })
    }

    fn lock(&self) -> MutexGuard<'_, HubState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn delay(&self) -> Duration {
        self.lock().delay
    }

    /// The live slot of a node (there is only one: `open_slot` closes the previous ones).
    pub(super) fn live_slot(&self, peer: PeerId) -> Option<u64> {
        self.lock().live_of(peer).first().copied()
    }

    pub(super) fn is_live(&self, slot: u64) -> bool {
        self.lock().slots.contains_key(&slot)
    }

    /// A new slot of a node; the live slots of the same node are closed (restart).
    pub(super) fn open_slot(&self, peer: PeerId) -> (u64, mpsc::Receiver<MemPath>) {
        let mut st = self.lock();
        for old in st.live_of(peer) {
            st.remove(old);
        }
        st.next_slot += 1;
        let id = st.next_slot;
        let (queue, rx) = mpsc::channel(ACCEPT_QUEUE);
        let lines = Vec::new();
        st.slots.insert(
            id,
            Slot {
                peer,
                queue,
                gone: watch::channel(false).0,
                lines,
            },
        );
        (id, rx)
    }

    pub(super) fn close_slot(&self, slot: u64) {
        self.lock().remove(slot);
    }

    /// A pair of paths, the target queue and its close signal; the queue is awaited outside the lock.
    pub(super) fn open_link(
        &self,
        from: u64,
        to: &PeerAddr,
    ) -> NetResult<(
        MemPath,
        MemPath,
        mpsc::Sender<MemPath>,
        watch::Receiver<bool>,
    )> {
        let hub = self.this.upgrade().ok_or(NetError::Closed)?;
        let mut st = self.lock();
        let me = st.slots.get(&from).ok_or(NetError::Closed)?.peer;
        let target = st.pick_target(self.net_id, to)?;
        if st.cuts.contains(&cut_key(me, to.id)) {
            return Err(NetError::NoRoute);
        }
        let (mine, theirs) = MemPath::pair(&hub, (me, from), (to.id, target));
        let tslot = st.slots.get(&target).ok_or(NetError::NoRoute)?;
        let (queue, gone) = (tslot.queue.clone(), tslot.gone.subscribe());
        for slot in [from, target] {
            if let Some(s) = st.slots.get_mut(&slot) {
                s.lines.retain(|w| w.strong_count() > 0);
                s.lines.push(Arc::downgrade(mine.line()));
            }
        }
        Ok((mine, theirs, queue, gone))
    }

    /// A discovery board with a name; one per (hub, name).
    pub(super) fn board(&self, name: &'static str) -> Arc<Board> {
        self.lock().boards.entry(name).or_default().clone()
    }

    fn set_cut(&self, a: PeerId, b: PeerId, on: bool) {
        let mut st = self.lock();
        let key = cut_key(a, b);
        if !on {
            st.cuts.remove(&key);
            return;
        }
        st.cuts.insert(key);
        let lines = st.slots.values().flat_map(|s| s.lines.iter());
        for line in lines.filter_map(Weak::upgrade).filter(|l| l.joins(a, b)) {
            line.close_all();
        }
    }

    fn set_delay(&self, delay: Duration) {
        self.lock().delay = delay;
    }
}

/// The `mem` network: the hub's handle, from which connectors, transports and discovery sources are taken.
pub struct MemNet {
    pub(super) hub: Arc<Hub>,
}

impl MemNet {
    pub fn new() -> Arc<MemNet> {
        Arc::new(MemNet { hub: Hub::new() })
    }

    /// One-way delay of every next packet on all paths.
    pub fn set_delay(&self, delay: Duration) {
        self.hub.set_delay(delay);
    }

    /// Breaks the live paths between nodes and forbids new ones (`NoRoute`).
    pub fn cut(&self, a: PeerId, b: PeerId) {
        self.hub.set_cut(a, b, true);
    }

    /// Lifts the `cut` ban; broken paths do not come back.
    pub fn heal(&self, a: PeerId, b: PeerId) {
        self.hub.set_cut(a, b, false);
    }

    /// A new connector of a node; the previous one of the same node is closed (restart).
    pub fn connector(&self, secret: &NodeSecret) -> Arc<MemConnector> {
        let peer = secret.peer_id();
        let (slot, rx) = self.hub.open_slot(peer);
        Arc::new(MemConnector::new(self.hub.clone(), slot, peer, rx))
    }

    /// Address of a node with a hint to its live slot; no slot (not started or closed) — `None`.
    pub fn addr_of(&self, peer: PeerId) -> Option<PeerAddr> {
        let slot = self.hub.live_slot(peer)?;
        Some(PeerAddr {
            id: peer,
            hints: vec![encode_hint(self.hub.net_id, slot)],
        })
    }

    /// A discovery source on the board named `name`.
    pub fn discovery(&self, name: &'static str, publishes: bool) -> Arc<MemDiscovery> {
        let board = self.hub.board(name);
        Arc::new(MemDiscovery::new(name, publishes, board))
    }
}

impl fmt::Debug for MemNet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MemNet {{ net_id: {} }}", self.hub.net_id)
    }
}

/// One `mem` network per process: the nodes of the tests of one binary share it, isolation is by network key and secret.
pub fn process_net() -> Arc<MemNet> {
    static NET: OnceLock<Arc<MemNet>> = OnceLock::new();
    NET.get_or_init(MemNet::new).clone()
}

#[cfg(test)]
#[path = "hub_tests.rs"]
mod tests;
