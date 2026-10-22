//! Forwarding a local TCP connection to somebody else's port (`forward_to_peer_port`) — the consumer side.
//!
//! What it does: checks that the node is a member, opens a ports service stream, sends
//! a request with the port number, reads the reply and, on `PORT_CODE_OK`, copies the bytes.
//! What it does not do: it does not listen on ports (that is `open`), does not retry, does not log a success.

use std::time::Duration;

use crate::net::{BoxRecv, BoxSend, NetStack, PeerId};
use anyhow::{anyhow, bail, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::frame::{port_code_text, encode_port_request, parse_port_reply, PORT_CODE_OK};
use super::types::Opened;
use crate::exit::{splice, PeerConnPool};
use crate::membership::Members;
use crate::output::say;

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
    if !members.is_member(target.node) {
        say("port", format!("{log_prefix}: node is offline"));
        return;
    }
    let reply_in_time = timeout(deadline, request_port_stream(stack, pool, target.node, target.port)).await;
    match reply_in_time {
        Err(_) => say(
            "port",
            format!("{log_prefix}: deadline expired ({} s)", deadline.as_secs()),
        ),
        Ok(Err(e)) => say("port", format!("{log_prefix}: {e}")),
        Ok(Ok((_, _, code))) if code != PORT_CODE_OK => {
            say("port", format!("{log_prefix}: {}", port_code_text(code)))
        }
        Ok(Ok((send, recv, _))) => {
            splice(send, recv, tcp).await;
        }
    }
}

/// Opens a stream, sends the request, reads the reply: the stream and the code.
async fn request_port_stream(
    stack: &NetStack,
    pool: &PeerConnPool,
    node: PeerId,
    port: u16,
) -> Result<(BoxSend, BoxRecv, u8)> {
    let crate::net::StreamPair { mut send, mut recv } = pool.open_peer_stream(stack, node).await?;
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
