//! The `ports` section of the snapshot: pure assembly of "my publications" and "network ports" per the port-publishing
//! rule — no network and no `.await`.

use std::collections::{HashMap, HashSet};

use crate::net::PeerId;

use crate::ports::open::OpenState;
use crate::ports::{LocalState, Opened, PortView, Published};
use crate::web::{NetPort, OpenedInfo, OwnPort, PortsInfo};

type Own = Vec<(Published, LocalState)>;
type OpenedNow = Vec<(Opened, OpenState)>;

fn local_text(s: LocalState) -> &'static str {
    match s {
        LocalState::Unknown => "unknown",
        LocalState::Ok => "ok",
        LocalState::Refused => "refused",
    }
}

fn opened_info(local: u16, st: &OpenState) -> OpenedInfo {
    match st {
        OpenState::Listening => OpenedInfo {
            local,
            state: "listening".into(),
            error: None,
        },
        OpenState::BindFailed(e) => OpenedInfo {
            local,
            state: "bind_failed".into(),
            error: Some(e.clone()),
        },
    }
}

fn find_opened(opened: &OpenedNow, node: PeerId, port: u16) -> Option<OpenedInfo> {
    opened
        .iter()
        .find(|(o, _)| o.node == node && o.port == port)
        .map(|(o, st)| opened_info(o.local, st))
}

/// The name of a record opened to node `own_id`, with a readable value; otherwise `None`.
fn public_name(v: &PortView, own_id: PeerId) -> Option<&str> {
    v.record.as_ref()?.name_for(&own_id)
}

fn row(
    node: PeerId,
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
        name: name.unwrap_or_default().to_string(),
        published: name.is_some(),
        opened,
    }
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
            name: p.name,
            local: local_text(s).into(),
        })
        .collect();
    let mut network = Vec::new();
    let mut seen: HashSet<(PeerId, u16)> = HashSet::new();
    for v in views.iter().filter(|v| v.author != own_id) {
        let op = find_opened(&opened, v.author, v.port);
        let name = public_name(v, own_id);
        if (name.is_some() && online.contains(&v.author)) || op.is_some() {
            seen.insert((v.author, v.port));
            network.push(row(v.author, v.port, name, &ctx, op));
        }
    }
    for (o, st) in opened.iter().filter(|(o, _)| o.node != own_id) {
        if seen.insert((o.node, o.port)) {
            network.push(row(
                o.node,
                o.port,
                None,
                &ctx,
                Some(opened_info(o.local, st)),
            ));
        }
    }
    PortsInfo { published, network }
}

#[cfg(test)]
#[path = "ports_tests.rs"]
mod tests;
