//! The reason for a grey icon when the page is ready: what the snapshot says about the node's network, and the menu words.

use super::NetLink;
use crate::app::snapshot::{TrayNodeStatus, TraySnapshot};
use crate::app::tray_texts::{
    net_connecting_status_text, net_lost_status_text, net_none_status_text, page_join_text,
    page_open_text,
};

/// For how many seconds after the node starts the `offline` status reads as "connecting" rather than "no connection"
/// (the connection to the relay server can take up to about a minute; the threshold has a margin).
/// A deliberate crudeness: a node that lost its connection in the first minute after start looks
/// like it is connecting in the menu. The opposite is worse: a person reads "no connection" for a minute although everything is fine, and goes digging in
/// the settings.
const CONNECTING_GRACE_SEC: u64 = 60;

/// What the `GET /api/state` snapshot says about the node's network. No `node`/`node.status` or `network: null` —
/// no network; no `network` key at all (an old snapshot) — there is a network (as in `color_facts`).
/// `offline` in the first `CONNECTING_GRACE_SEC` seconds of the node running is still connecting; with no
/// `node.uptime_sec` the age is unknown, and `offline` stays a lost connection.
pub fn net_link(snap: &TraySnapshot) -> NetLink {
    let Some(status) = snap.node.as_ref().and_then(|n| n.status) else {
        return NetLink::NoNetwork;
    };
    if snap.network_is_null {
        return NetLink::NoNetwork;
    }
    match status {
        TrayNodeStatus::Online => NetLink::Joined,
        TrayNodeStatus::Starting | TrayNodeStatus::Stopping => NetLink::Connecting,
        TrayNodeStatus::Offline if just_started(snap) => NetLink::Connecting,
        TrayNodeStatus::Offline | TrayNodeStatus::Unknown => NetLink::Lost,
    }
}

/// The node has been running for less than `CONNECTING_GRACE_SEC` seconds (by the snapshot's `node.uptime_sec`).
fn just_started(snap: &TraySnapshot) -> bool {
    let uptime = snap.node.as_ref().and_then(|n| n.uptime_sec);
    uptime.is_some_and(|sec| sec < CONNECTING_GRACE_SEC)
}

/// The first menu line for a grey icon, a ready page and a running node.
pub(super) fn grey_status(net: NetLink) -> String {
    match net {
        NetLink::NoNetwork => net_none_status_text(),
        NetLink::Connecting | NetLink::Joined => net_connecting_status_text(),
        NetLink::Lost => net_lost_status_text(),
    }
}

/// The second menu item in the same view: join a network if there is none, otherwise open the page.
pub(super) fn grey_page_text(net: NetLink) -> String {
    match net {
        NetLink::NoNetwork => page_join_text(),
        _ => page_open_text(),
    }
}

#[cfg(test)]
#[path = "view_net_tests.rs"]
mod tests;
