//! Where a relay pushes client datagrams and where replies come from. Two traits instead of an enum: a relay has
//! two independent pumps and is generic over both, so the route kind is decided in one place (`serve`).

use std::future::Future;
use std::net::SocketAddr;

use crate::exit::udpgw_codec as codec;
use crate::proxy::socks5::UdpPacket;
use crate::transport::net::{BoxRecv, BoxSend};
use tokio::io::AsyncWriteExt;

use super::bridge::{from_gateway_frame, to_gateway_frame};

/// What happened to a datagram handed to the upstream.
pub(super) enum Delivery {
    Sent,
    /// Does not fit the frame of the gateway.
    Oversize,
    /// No address to send to.
    Unroutable,
}

/// The upstream is closed: the association ends.
pub(super) struct Closed;

pub(super) trait UpstreamTx: Send {
    fn send(&mut self, packet: UdpPacket) -> impl Future<Output = Result<Delivery, Closed>> + Send;
}

pub(super) trait UpstreamRx: Send {
    /// The next reply with its source; `None` — the upstream is closed.
    fn recv(&mut self) -> impl Future<Output = Option<(SocketAddr, Vec<u8>)>> + Send;
}

/// The write half of the stream to the UDP gateway of an exit.
pub(super) struct ExitTx(pub BoxSend);

/// The read half of the stream to the UDP gateway of an exit.
pub(super) struct ExitRx(pub BoxRecv);

impl UpstreamTx for ExitTx {
    async fn send(&mut self, packet: UdpPacket) -> Result<Delivery, Closed> {
        let Some(bytes) = to_gateway_frame(packet) else {
            return Ok(Delivery::Oversize);
        };
        self.0.write_all(&bytes).await.map_err(|_| Closed)?;
        Ok(Delivery::Sent)
    }
}

impl UpstreamRx for ExitRx {
    async fn recv(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        loop {
            let frame = codec::read_frame(&mut self.0).await.ok()?;
            if let Some(reply) = from_gateway_frame(frame) {
                return Some(reply);
            }
        }
    }
}
