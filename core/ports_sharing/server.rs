//! Handler of incoming connections on the publishing side (`meshtown/port/1`).
//!
//! What it does: for each stream — the permission check (a member and a published port), the request frame,
//! a connection to our own loopback and copying bytes.
//! What it does not do: it does not write to the document (that is `announce`), it refuses nodes outside the `Nodes` list.
//! Links: the permission check — `Members`, the list — `PublishedPortsTable`, bytes — `crate::exit::splice`.

use std::net::SocketAddr;
use std::time::Duration;

use std::sync::Arc;

use crate::transport::net::{
    BoxFuture, BoxRecv, BoxSend, PeerConn, PeerId, ServiceHandler, StreamPair,
};
use anyhow::{anyhow, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::frame::{
    encode_port_reply, parse_port_request, PORT_CODE_BAD_REQUEST, PORT_CODE_CONNECT_FAILED,
    PORT_CODE_NOT_PUBLISHED, PORT_CODE_OK, PORT_CODE_REFUSED,
};
use super::table::{LocalState, PublishedPortsTable};
use super::types::Audience;
use crate::exit::splice;
use crate::members::membership::Members;
use crate::node::output::say;
use crate::node::tcp_tune::disable_nagle;

/// Overall deadline of the local connection (both addresses together).
pub const LOCAL_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Deadline for waiting for the request frame.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Handler of incoming connections of the `PORT` service; it does not hold the stack (a clone keeps the last one from leaving).
#[derive(Clone)]
pub struct PortServer {
    pub members: Members,
    pub table: PublishedPortsTable,
}

/// Ports handler for `NetStack`.
pub fn port_service(members: Members, table: PublishedPortsTable) -> Arc<dyn ServiceHandler> {
    Arc::new(PortServer { members, table })
}

impl ServiceHandler for PortServer {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        Box::pin(serve_conn(self.clone(), conn))
    }
}

/// Accepts the connection's streams; any `accept` error is a quiet end of the loop.
async fn serve_conn(server: PortServer, conn: Arc<dyn PeerConn>) {
    let peer = conn.peer();
    while let Ok(pair) = conn.accept().await {
        spawn_stream(&server, peer, pair);
    }
}

/// Its own stream for each request — outside the accept loop.
fn spawn_stream(server: &PortServer, peer: PeerId, pair: StreamPair) {
    let server = server.clone();
    tokio::spawn(async move {
        if let Err(e) = serve_stream(server, peer, pair.send, pair.recv).await {
            say(
                "port",
                format!(
                    "{}: stream closed: {e:#}",
                    crate::node::output::short_peer_id(&peer)
                ),
            );
        }
    });
}

/// The right to a port: not a member — refusal; no publication — "not published"; `All` — admits;
/// `Nodes` — admits a node from the list, refuses the rest, and everyone if the list is empty.
pub(crate) fn admit_port_request(
    is_member: bool,
    audience: Option<&Audience>,
    peer: &PeerId,
) -> Result<(), u8> {
    if !is_member {
        return Err(PORT_CODE_REFUSED);
    }
    match audience {
        None => Err(PORT_CODE_NOT_PUBLISHED),
        Some(a) if a.includes(peer) => Ok(()),
        Some(_) => Err(PORT_CODE_REFUSED),
    }
}

/// A connection to our own loopback: `127.0.0.1`, on refusal `[::1]`; the deadline is shared.
pub async fn connect_loopback_port(port: u16) -> Option<TcpStream> {
    let both = async {
        let v4 = SocketAddr::from(([127, 0, 0, 1], port));
        if let Ok(tcp) = TcpStream::connect(v4).await {
            disable_nagle(&tcp);
            return Some(tcp);
        }
        let tcp = TcpStream::connect(SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port)))
            .await
            .ok()?;
        disable_nagle(&tcp);
        Some(tcp)
    };
    timeout(LOCAL_CONNECT_TIMEOUT, both).await.ok().flatten()
}

async fn read_port_request(recv: &mut BoxRecv) -> Option<u16> {
    let mut buf = [0u8; 3];
    timeout(REQUEST_TIMEOUT, recv.read_exact(&mut buf))
        .await
        .ok()?
        .ok()?;
    parse_port_request(&buf).ok()
}

async fn send_reply_code(send: &mut BoxSend, code: u8) -> Result<()> {
    send.write_all(&encode_port_reply(code))
        .await
        .map_err(|e| anyhow!("reply not sent: {e}"))?;
    Ok(())
}

/// Refusal: a `[port]` log line and a reply with the code.
async fn refuse(
    send: &mut BoxSend,
    peer: &PeerId,
    members: &Members,
    why: &str,
    code: u8,
) -> Result<()> {
    say("port", format!("{}: {why}", members.peer_label(*peer)));
    send_reply_code(send, code).await
}

/// Answers one request and, if everything checks out, copies bytes.
async fn serve_stream(
    server: PortServer,
    peer: PeerId,
    mut send: BoxSend,
    mut recv: BoxRecv,
) -> Result<()> {
    let Some(port) = read_port_request(&mut recv).await else {
        return refuse(
            &mut send,
            &peer,
            &server.members,
            "request could not be parsed",
            PORT_CODE_BAD_REQUEST,
        )
        .await;
    };
    let audience = server.table.audience_of(port);
    if let Err(code) = admit_port_request(server.members.is_member(peer), audience.as_ref(), &peer)
    {
        let why = format!("port {port}: {}", super::frame::port_code_text(code));
        return refuse(&mut send, &peer, &server.members, &why, code).await;
    }
    let Some(tcp) = connect_loopback_port(port).await else {
        server.table.set_local_state(port, LocalState::Refused);
        let why = format!("port {port} does not respond");
        return refuse(
            &mut send,
            &peer,
            &server.members,
            &why,
            PORT_CODE_CONNECT_FAILED,
        )
        .await;
    };
    send_reply_code(&mut send, PORT_CODE_OK).await?;
    server.table.set_local_state(port, LocalState::Ok);
    say(
        "port",
        format!("{}: port {port} opened", server.members.peer_label(peer)),
    );
    let _ = splice(send, recv, tcp).await;
    Ok(())
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "server_stream_tests.rs"]
mod stream_tests;
