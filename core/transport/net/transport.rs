//! Connection to a node (`PeerConn`) and a whole transport (`Transport`).
//! Assembling a transport from a connector and a carrier is in `split`, only traits are here.

use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::error::NetResult;
use crate::transport::net::id::PeerId;
use crate::transport::net::link::{LinkInfo, NetInfo, PathChange};
use crate::transport::net::service::ServiceId;
use crate::transport::net::stream::{BoxFuture, StreamPair};
use std::sync::Arc;

/// Stream of chosen-path changes of one connection.
pub trait PathWatch: Send {
    /// Next change of the chosen path; `None` — the connection is closed or the transport does not track the path,
    /// only `None` after that.
    fn next(&mut self) -> BoxFuture<'_, Option<PathChange>>;
}

/// Default observer: there are no changes.
pub struct NoPathWatch;

impl PathWatch for NoPathWatch {
    fn next(&mut self) -> BoxFuture<'_, Option<PathChange>> {
        Box::pin(async { None })
    }
}

/// Reason the transport closed a connection because the OS network changed; on `NET_CHANGED` the member is redialed at once.
pub const NET_CHANGED: &str = "network changed";
/// Reason the transport closed a connection because the OS network disappeared.
pub const NET_LOST: &str = "network lost";

pub trait PeerConn: Send + Sync {
    /// Verified peer.
    fn peer(&self) -> PeerId;
    fn stable_id(&self) -> u64;
    fn open(&self) -> BoxFuture<'_, NetResult<StreamPair>>;
    /// `Closed` after closing.
    fn accept(&self) -> BoxFuture<'_, NetResult<StreamPair>>;
    fn link(&self) -> LinkInfo;
    fn close(&self, code: u32, reason: &[u8]);
    /// Fires on both sides.
    fn closed(&self) -> BoxFuture<'_, ()>;
    /// Returns whether the connection is closed (after `close` from either side — `true`, not earlier).
    /// The implementor must override it: the default `false` is fit only for stubs.
    fn is_closed(&self) -> bool {
        false
    }
    /// Access to the concrete connection type for downcasting.
    /// The implementor returns `self`; the default returns `()` and exposes nothing.
    fn as_any(&self) -> &dyn std::any::Any {
        &()
    }
    /// Description of the chosen path with rtt; empty if the transport does not know.
    fn path_text(&self) -> String {
        String::new()
    }
    /// Congestion statistics of the chosen path; empty if the transport does not know.
    fn congestion_text(&self) -> String {
        String::new()
    }
    /// One line per path: `{address}, chosen|spare: {congestion}`.
    fn path_lines(&self) -> Vec<String> {
        Vec::new()
    }
    /// Close reason as text; empty until closed or unknown.
    fn close_reason(&self) -> String {
        String::new()
    }
    /// Observer of chosen-path changes.
    fn path_watch(&self) -> Box<dyn PathWatch> {
        Box::new(NoPathWatch)
    }
}

pub trait Transport: Send + Sync {
    fn kind(&self) -> &'static str;
    fn me(&self) -> PeerId;
    fn local_addr(&self) -> PeerAddr;
    fn can_reach(&self, to: &PeerAddr) -> bool;
    /// An address of `peer` this transport itself dialed successfully — fit for an invite and the address cache.
    fn proven_addr(&self, _peer: PeerId) -> Option<std::net::SocketAddr> {
        None
    }
    fn add_hints(&self, addr: &PeerAddr);
    fn set_relays(&self, relays: &[RelayRef]);
    /// Called before the first `accept` (the list of services to accept).
    fn serve(&self, svcs: &[ServiceId]);
    fn connect(&self, to: &PeerAddr, svc: ServiceId)
        -> BoxFuture<'_, NetResult<Arc<dyn PeerConn>>>;
    /// Only the services being served; `None` — the transport is closed.
    fn accept(&self) -> BoxFuture<'_, Option<(ServiceId, Arc<dyn PeerConn>)>>;
    fn net_info(&self) -> NetInfo;
    /// Waits for the first network report (the implementation has its own wait ceiling); the default is ready at once.
    fn wait_net_info(&self) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
    fn close(&self) -> BoxFuture<'_, ()>;
}
