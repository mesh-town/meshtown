//! Connectivity: how to find and punch a path to a node (`Connector`) and the datagram path itself (`PacketPath`).
//! There is no data stream or encryption over the path here — that is `carrier`.

use crate::transport::net::addr::{PeerAddr, RelayRef};
use crate::transport::net::error::NetResult;
use crate::transport::net::id::PeerId;
use crate::transport::net::link::{NetInfo, PathKind};
use crate::transport::net::stream::BoxFuture;
use bytes::Bytes;
use std::any::Any;
use std::net::SocketAddr;

/// Path details; `peer` is verified by the connector.
#[derive(Clone, Debug)]
pub struct PathInfo {
    pub peer: PeerId,
    pub kind: PathKind,
    pub remote: Option<SocketAddr>,
    /// Socket kind: `"mem"`, in the future `"udx"`, `"udp"`.
    pub family: &'static str,
}

/// Two-way datagram path to a verified node.
pub trait PacketPath: Send + Sync {
    fn info(&self) -> PathInfo;
    fn max_packet_bytes(&self) -> usize;
    /// A datagram; on real paths loss is allowed.
    fn send(&self, pkt: Bytes) -> BoxFuture<'_, std::io::Result<()>>;
    /// `UnexpectedEof` — the path is closed.
    fn recv(&self) -> BoxFuture<'_, std::io::Result<Bytes>>;
    fn close(&self);
    /// Access to the native socket for a carrier of the same family.
    fn as_any(&self) -> &dyn Any;
}

/// Connectivity: path search and punching. A foreign key at the address gives `WrongPeer`, no path gives `NoRoute`.
pub trait Connector: Send + Sync {
    fn kind(&self) -> &'static str;
    fn local_addr(&self) -> PeerAddr;
    fn can_reach(&self, to: &PeerAddr) -> bool;
    fn add_hints(&self, addr: &PeerAddr);
    fn set_relays(&self, relays: &[RelayRef]);
    fn dial(&self, to: &PeerAddr) -> BoxFuture<'_, NetResult<Box<dyn PacketPath>>>;
    /// Incoming path; `None` — the connector is closed.
    fn listen(&self) -> BoxFuture<'_, Option<Box<dyn PacketPath>>>;
    fn net_info(&self) -> NetInfo;
    fn close(&self) -> BoxFuture<'_, ()>;
}
