//! Data stream over a ready path: `Carrier` builds a `PeerConn` from a `PacketPath`.
//! There is no path search or punching here — that is `connect`.

use crate::net::connect::{PacketPath, PathInfo};
use crate::net::error::NetResult;
use crate::net::service::ServiceId;
use crate::net::stream::BoxFuture;
use crate::net::transport::PeerConn;
use std::sync::Arc;

/// Role of a side when a connection is started on a path.
pub enum CarrierRole {
    /// Dialer: sends the `ServiceId` first thing.
    Dial(ServiceId),
    /// Acceptor: checks the received `ServiceId` against the list.
    Accept(Arc<[ServiceId]>),
}

pub trait Carrier: Send + Sync {
    fn kind(&self) -> &'static str;
    /// Returns whether the carrier fits this path (for example, only the native socket of its own family).
    fn can_carry(&self, path: &PathInfo) -> bool;
    /// A foreign service at the acceptor gives `Refused`, and the path is closed.
    fn start(
        &self,
        path: Box<dyn PacketPath>,
        role: CarrierRole,
    ) -> BoxFuture<'_, NetResult<(ServiceId, Arc<dyn PeerConn>)>>;
}
