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
    encode_port_request, parse_port_reply, port_code_text, PORT_CODE_CONNECT_FAILED,
    PORT_CODE_NOT_PUBLISHED, PORT_CODE_OK, PORT_CODE_REFUSED,
};
use super::types::Opened;
use crate::exit::{splice, PeerConnPool};
use crate::members::membership::Members;
use crate::node::output::say;

/// Deadline for the dial, the request and the reply; when it expires, the TCP connection is closed.
pub const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Forwards an accepted TCP connection to a member's port.
pub async fn forward_to_peer_port(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    target: &Opened,
    tcp: TcpStream,
) {
    forward_to_peer_port_within(stack, pool, members, target, tcp, DIAL_TIMEOUT).await
}

/// The same with its own deadline — for fast tests. Any refusal closes `tcp` and writes one `[port]` line.
pub(crate) async fn forward_to_peer_port_within(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    target: &Opened,
    tcp: TcpStream,
    deadline: Duration,
) {
    let log_prefix = format!(
        "127.0.0.1:{} → {}:{}",
        target.local,
        members.peer_label(target.node),
        target.port
    );
    match dial_peer_port(stack, pool, members, target.node, target.port, deadline).await {
        Err(e) => say("port", format!("{log_prefix}: {}", e.text)),
        Ok(stream) => {
            splice(stream.send, stream.recv, tcp).await;
        }
    }
}

/// An open ports stream to a member's port, after a successful reply.
pub struct PortStream {
    pub send: BoxSend,
    pub recv: BoxRecv,
}

/// Why a port could not be reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Opens a ports stream to `node:port` and reads the reply, within `deadline`. Does not retry, does not log.
pub async fn dial_peer_port(
    stack: &NetStack,
    pool: &PeerConnPool,
    members: &Members,
    node: PeerId,
    port: u16,
    deadline: Duration,
) -> Result<PortStream, PortDialError> {
    if !members.is_member(node) {
        return Err(PortDialError::new(PortReach::Offline, "member is offline"));
    }
    let reply = timeout(deadline, request_port_stream(stack, pool, node, port)).await;
    match reply {
        Err(_) => Err(timed_out(deadline)),
        Ok(Err(e)) => Err(PortDialError::new(PortReach::Failed, e.to_string())),
        Ok(Ok((_, _, code))) if code != PORT_CODE_OK => Err(refusal(code)),
        Ok(Ok((send, recv, _))) => Ok(PortStream { send, recv }),
    }
}

fn timed_out(deadline: Duration) -> PortDialError {
    let text = format!("deadline expired ({} s)", deadline.as_secs());
    PortDialError::new(PortReach::TimedOut, text)
}

fn refusal(code: u8) -> PortDialError {
    let reach = match code {
        PORT_CODE_NOT_PUBLISHED | PORT_CODE_REFUSED => PortReach::NotOpened,
        PORT_CODE_CONNECT_FAILED => PortReach::NoService,
        _ => PortReach::Failed,
    };
    PortDialError::new(reach, port_code_text(code))
}

/// Opens a stream, sends the request, reads the reply: the stream and the code.
async fn request_port_stream(
    stack: &NetStack,
    pool: &PeerConnPool,
    node: PeerId,
    port: u16,
) -> Result<(BoxSend, BoxRecv, u8)> {
    let crate::transport::net::StreamPair { mut send, mut recv } =
        pool.open_peer_stream(stack, node).await?;
    send.write_all(&encode_port_request(port))
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
