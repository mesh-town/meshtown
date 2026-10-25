//! The refusal a responder gives at the handshake: the typed error context and how an initiator learns it.
//!
//! What it does: reads the close code of a connection the responder dropped, notes it in the access view and
//! attaches it to the handshake error as `RefusedByPeer`; logs once per run that the list removed this node.
//! What it doesn't do: no handshake exchange itself (see `handshake`).

use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;

use crate::members::access::view::{AccessView, Deny, Side};
use crate::members::membership::MemberContext;
use crate::node::output::{say, NET_NAME};
use crate::transport::net::PeerConn;

/// Context of a handshake error: the responder closed the connection with this refusal.
#[derive(Debug, Clone, Copy)]
pub struct RefusedByPeer(pub Deny);

impl std::fmt::Display for RefusedByPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "peer refused us: {}", self.0.code())
    }
}

/// The responder did not answer: if it closed with a refusal, remember it (the close frame may arrive a moment later).
pub(super) async fn learn_refusal(conn: &Arc<dyn PeerConn>, ctx: &MemberContext) -> Option<Deny> {
    let _ = timeout(Duration::from_secs(1), conn.closed()).await;
    let reason = Deny::from_wire(&conn.close_reason());
    let access = ctx.members.access();
    reason.iter().for_each(|deny| {
        access.note_refusal(conn.peer(), Side::Them, *deny);
        warn_if_removed(&access, *deny);
    });
    reason
}

/// Logs the "removed from the list" notice once per run.
fn warn_if_removed(access: &AccessView, deny: Deny) {
    if deny != Deny::Forgotten || !access.first_removed_notice() {
        return;
    }
    say(
        "access",
        format!("acl: members removed this computer from the member list; join again with an invite on the {NET_NAME} page"),
    );
}

/// A failed exchange: if the responder's refusal is not attached yet, learns it from the close frame.
pub(super) async fn typed_refusal(
    err: anyhow::Error,
    conn: &Arc<dyn PeerConn>,
    ctx: &MemberContext,
) -> anyhow::Error {
    match err.downcast_ref::<RefusedByPeer>() {
        Some(_) => err,
        None => with_refusal(err, learn_refusal(conn, ctx).await),
    }
}

/// Attaches the responder's refusal, if any, to the error as a typed context.
pub(super) fn with_refusal(err: anyhow::Error, refusal: Option<Deny>) -> anyhow::Error {
    match refusal {
        Some(deny) => err.context(RefusedByPeer(deny)),
        None => err,
    }
}
