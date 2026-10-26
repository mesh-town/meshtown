//! Forwarding a local TCP connection to somebody else's port (`forward_to_peer_port`) — the consumer side.
//!
//! What it does: checks that the node is a member, opens a ports service stream, sends
//! a request with the port number, reads the reply and, on `PORT_CODE_OK`, copies the bytes.
//! What it does not do: it does not listen on ports (that is `open`), does not retry, does not log a success.

use std::time::Duration;

use crate::transport::net::{BoxRecv, BoxSend, NetStack, PeerId};
use anyhow::{anyhow, bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::frame::{
    encode_port_request, parse_port_reply, port_code_text, PortRequest, PORT_CODE_BAD_REQUEST,
    PORT_CODE_CONNECT_FAILED, PORT_CODE_NOT_PUBLISHED, PORT_CODE_OK, PORT_CODE_REFUSED,
};
use super::types::{Opened, Proto};
use crate::exit::{splice, PeerConnPool};
use crate::members::membership::Members;
use crate::node::output::say;

/// Deadline for the dial, the request and the reply; when it expires, the TCP connection is closed.
pub const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Forwards an accepted TCP connection to a member's port. A refusal closes `tcp`, writes one `[port]`
/// line and is returned to the caller (the listener keeps it as the last dial error). `on_dialed` runs as soon
/// as the peer has accepted the stream, before the bytes are copied.
pub async fn forward_to_peer_port(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    target: &Opened,
    tcp: TcpStream,
    on_dialed: impl FnOnce(),
) -> Result<(), PortDialError> {
    forward_result(stack, pool, members, target, tcp, DIAL_TIMEOUT, on_dialed).await
}

/// The same with its own deadline and without the result — for fast tests.
#[cfg(test)]
pub(crate) async fn forward_to_peer_port_within(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    target: &Opened,
    tcp: TcpStream,
    deadline: Duration,
) {
    let _ = forward_result(stack, pool, members, target, tcp, deadline, || {}).await;
}

async fn forward_result(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    target: &Opened,
    tcp: TcpStream,
    deadline: Duration,
    on_dialed: impl FnOnce(),
) -> Result<(), PortDialError> {
    let log_prefix = format!(
        "127.0.0.1:{} → {}:{}",
        target.local,
        members.peer_label(target.node),
        target.port
    );
    match dial_peer_port(stack, pool, members, target.node, target.port, deadline).await {
        Err(e) => {
            say("port", format!("{log_prefix}: {}", e.text));
            Err(e)
        }
        Ok(stream) => {
            on_dialed();
            splice(stream.send, stream.recv, tcp).await;
            Ok(())
        }
    }
}

/// An open ports stream to a member's port, after a successful reply.
pub struct PortStream {
    pub send: BoxSend,
    pub recv: BoxRecv,
}

/// Why a port could not be reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortReach {
    /// The node is not a member.
    Offline,
    /// The node does not publish the port or does not open it for us.
    NotOpened,
    /// The node is there, but the service behind the port does not respond.
    NoService,
    /// No reply within the deadline.
    TimedOut,
    /// Any other failure.
    Failed,
    /// The node does not support UDP ports.
    Unsupported,
}

/// A refusal: its class and the human-readable text.
#[derive(Clone, Debug)]
pub struct PortDialError {
    pub reach: PortReach,
    pub text: String,
}

impl PortDialError {
    fn new(reach: PortReach, text: impl Into<String>) -> Self {
        PortDialError {
            reach,
            text: text.into(),
        }
    }
}

/// Opens a TCP ports stream to `node:port` and reads the reply, within `deadline`.
pub async fn dial_peer_port(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    node: PeerId,
    port: u16,
    deadline: Duration,
) -> Result<PortStream, PortDialError> {
    let req = PortRequest {
        proto: Proto::Tcp,
        port,
    };
    dial_peer(stack, pool, members, node, req, deadline).await
}

/// Opens a ports stream for `req` to `node` and reads the reply, within `deadline`. Does not retry, does not log.
pub async fn dial_peer(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    node: PeerId,
    req: PortRequest,
    deadline: Duration,
) -> Result<PortStream, PortDialError> {
    if !members.is_member(node) {
        return Err(PortDialError::new(PortReach::Offline, "member is offline"));
    }
    let reply = timeout(deadline, request_port_stream(stack, pool, node, req)).await;
    match reply {
        Err(_) => Err(timed_out(deadline)),
        Ok(Err(e)) => Err(PortDialError::new(PortReach::Failed, e.to_string())),
        Ok(Ok((_, _, code))) if code != PORT_CODE_OK => Err(refusal(code, req.proto)),
        Ok(Ok((send, recv, _))) => Ok(PortStream { send, recv }),
    }
}

fn timed_out(deadline: Duration) -> PortDialError {
    let text = format!("deadline expired ({} s)", deadline.as_secs());
    PortDialError::new(PortReach::TimedOut, text)
}

/// A node that answers a UDP request with "bad request" does not know the UDP frame.
const UNSUPPORTED_UDP_TEXT: &str = "the node does not support UDP ports (it runs an older version)";

fn refusal(code: u8, proto: Proto) -> PortDialError {
    let reach = match code {
        PORT_CODE_NOT_PUBLISHED | PORT_CODE_REFUSED => PortReach::NotOpened,
        PORT_CODE_CONNECT_FAILED => PortReach::NoService,
        PORT_CODE_BAD_REQUEST if !proto.is_tcp() => {
            return PortDialError::new(PortReach::Unsupported, UNSUPPORTED_UDP_TEXT);
        }
        _ => PortReach::Failed,
    };
    PortDialError::new(reach, port_code_text(code))
}

/// Opens a stream, sends the request, reads the reply: the stream and the code.
async fn request_port_stream(
    stack: &NetStack,
    pool: &PeerConnPool,
    node: PeerId,
    req: PortRequest,
) -> Result<(BoxSend, BoxRecv, u8)> {
    let crate::transport::net::StreamPair { mut send, mut recv } =
        pool.open_peer_stream(stack, node).await?;
    send.write_all(&encode_port_request(req))
        .await
        .map_err(|e| anyhow!("request not sent: {e}"))?;
    let mut answer = [0u8; 2];
    recv.read_exact(&mut answer)
        .await
        .map_err(|e| anyhow!("reply not received: {e}"))?;
    match parse_port_reply(&answer) {
        Ok(code) => Ok((send, recv, code)),
        Err(e) => bail!("{e}"),
    }
}

#[cfg(test)]
#[path = "dial_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dial_forward_tests.rs"]
mod forward_tests;

#[cfg(test)]
#[path = "dial_peer_tests.rs"]
mod dial_peer_tests;

#[cfg(test)]
#[path = "dial_udp_tests.rs"]
mod udp_tests;
