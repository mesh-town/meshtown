//! The `ports` section of the snapshot: pure assembly of "my publications" and "network ports" per the port-publishing
//! rule — no network and no `.await`.

use std::collections::{HashMap, HashSet};

use crate::proxy::MeshAddress;
use crate::transport::net::PeerId;

use crate::ports_sharing::open::OpenState;
use crate::ports_sharing::{LocalState, Opened, PortReach, PortView, Proto, Published};
use crate::web_gui::{NetPort, OpenedInfo, OwnPort, PortsInfo};

type Own = Vec<(Published, LocalState)>;
type OpenedNow = Vec<(Opened, OpenState, Option<PortReach>)>;

fn local_text(s: LocalState) -> &'static str {
    match s {
        LocalState::Unknown => "unknown",
        LocalState::Ok => "ok",
        LocalState::Refused => "refused",
    }
}

/// A UDP port cannot be probed locally, so its state is always unknown.
fn own_local(proto: Proto, s: LocalState) -> LocalState {
    match proto {
        Proto::Udp => LocalState::Unknown,
        Proto::Tcp => s,
    }
}

fn opened_info(local: u16, st: &OpenState, last_error: Option<PortReach>) -> OpenedInfo {
    match st {
        OpenState::Listening => OpenedInfo {
            local,
            state: "listening".into(),
            error: None,
            last_error,
        },
        OpenState::BindFailed(e) => OpenedInfo {
            local,
            state: "bind_failed".into(),
            error: Some(e.clone()),
            last_error,
        },
    }
}

fn find_opened(opened: &OpenedNow, node: PeerId, proto: Proto, port: u16) -> Option<OpenedInfo> {
    opened
        .iter()
        .find(|(o, _, _)| o.node == node && o.proto == proto && o.port == port)
        .map(|(o, st, reach)| opened_info(o.local, st, *reach))
}

/// The name of a record opened to node `own_id`, with a readable value; otherwise `None`.
fn public_name(v: &PortView, own_id: PeerId) -> Option<&str> {
    v.record.as_ref()?.name_for(&own_id)
}

fn row(
    node: PeerId,
    proto: Proto,
    port: u16,
    name: Option<&str>,
    ctx: &Ctx<'_>,
    opened: Option<OpenedInfo>,
) -> NetPort {
    NetPort {
        node: node.to_string(),
        node_name: ctx.names.get(&node).cloned(),
        online: ctx.online.contains(&node),
        port,
        proto,
        name: name.unwrap_or_default().to_string(),
        published: name.is_some(),
        opened,
        web_url: None,
    }
}

/// Sets `web_url` on every shared port whose author has a mesh address.
pub(in crate::node::node_ctl) fn overlay_web_urls(rows: &mut [NetPort], addresses: &[MeshAddress]) {
    rows.iter_mut()
        .for_each(|row| row.web_url = web_url_of(row, addresses));
}

fn web_url_of(row: &NetPort, addresses: &[MeshAddress]) -> Option<String> {
    let address = addresses.iter().find(|a| a.peer.to_string() == row.node)?;
    (row.published && row.proto == Proto::Tcp).then(|| address.url(row.port))
}

struct Ctx<'a> {
    names: &'a HashMap<PeerId, String>,
    online: &'a HashSet<PeerId>,
}

/// Builds the `ports` section: other members' records opened to this node, with a readable value — if the author is on the network
/// or the port is open here; an open port without a usable record — a row with `published: false`;
/// own records do not go into `network`.
pub(super) fn build_ports(
    views: &[PortView],
    own_id: PeerId,
    names: &HashMap<PeerId, String>,
    online: &HashSet<PeerId>,
    own: Own,
    opened: OpenedNow,
) -> PortsInfo {
    let ctx = Ctx { names, online };
    let published = own
        .into_iter()
        .map(|(p, s)| OwnPort {
            port: p.port,
            proto: p.proto,
            name: p.name,
            local: local_text(own_local(p.proto, s)).into(),
        })
        .collect();
    let mut network = Vec::new();
    let mut seen: HashSet<(PeerId, Proto, u16)> = HashSet::new();
    for v in views.iter().filter(|v| v.author != own_id) {
        let op = find_opened(&opened, v.author, v.proto, v.port);
        let name = public_name(v, own_id);
        if (name.is_some() && online.contains(&v.author)) || op.is_some() {
            seen.insert((v.author, v.proto, v.port));
            network.push(row(v.author, v.proto, v.port, name, &ctx, op));
        }
    }
    for (o, st, reach) in opened.iter().filter(|(o, _, _)| o.node != own_id) {
        if seen.insert((o.node, o.proto, o.port)) {
            network.push(row(
                o.node,
                o.proto,
                o.port,
                None,
                &ctx,
                Some(opened_info(o.local, st, *reach)),
            ));
        }
    }
    PortsInfo { published, network }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
