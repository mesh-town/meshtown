//! Core of the network stack: types without third-party network libraries (no logic here).
//! Map: `id` — PeerId/NodeSecret; `addr` — addresses and hints; `service` — ServiceId;
//! `link` — path and network information; `error` — NetError; `replica` — record and replica trait;
//! `sync_backend` — sync (membership and replicas) and its slot; `task_label` — per-task network label.

pub mod addr;
pub mod carrier;
pub mod connect;
#[cfg(test)]
pub(crate) mod contract;
pub mod discovery;
pub mod discovery_mux;
#[cfg(test)]
pub(crate) mod endpoint_shim;
pub mod error;
pub mod facade;
pub mod id;
pub mod link;
pub mod mem;
pub mod registry;
pub mod replica;
pub mod service;
pub mod split;
pub mod stream;
pub mod sync;
pub mod sync_backend;
pub mod task_label;
pub mod transport;

pub use addr::{AddrHint, PeerAddr, RelayRef};
pub use carrier::{Carrier, CarrierRole};
pub use connect::{Connector, PacketPath, PathInfo};
pub use discovery::{Discovery, LocalAddr};
pub use discovery_mux::{DiscoveryMux, HINT_TTL};
pub use error::{NetError, NetResult};
pub use facade::{NetSnapshot, NetStack, NetStackBuilder, ServiceHandler, WeakStack};
pub use id::{IdError, NodeSecret, PeerId};
pub use link::{LinkInfo, NatKind, NetInfo, PathChange, PathKind};
pub use mem::{MemConnector, MemDiscovery, MemNet};
pub use registry::{DiscoveryCtor, NetChoice, NetCtx, Registry, TransportCtor};
pub use replica::{
    live, Replica, ReplicaEntry, ReplicaEvent, ReplicaId, SyncCount, MAX_VALUE_BYTES,
};
pub use service::{ServiceId, AUTH, EXIT, INVITE, PORT, REPLICA, SYNC};
pub use split::SplitTransport;
pub use stream::{read_to_end_limited, BoxFuture, BoxRecv, BoxSend, StreamPair};
pub use sync::{Membership, SwarmEvent};
pub use sync_backend::{Swarm, SyncBackend, SyncCtor, SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
pub use transport::{NoPathWatch, PathWatch, PeerConn, Transport, NET_CHANGED, NET_LOST};
