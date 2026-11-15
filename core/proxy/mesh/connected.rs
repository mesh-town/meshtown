//! The page of the bare `meshtown` name: which devices have opened ports to this node.

use std::collections::HashMap;

use tokio::time::timeout;

use crate::config::config_file::NetSlotId;
use crate::ports_sharing::{PortView, PortsDoc, Proto};
use crate::proxy::front::NetEntry;
use crate::transport::net::PeerId;

use super::catalog::{addresses, Device};
use super::host::mesh_url;
use super::open_page::DOC_READ_TIMEOUT;
use super::page::{OpenPort, PageKind, SharedDevice};

/// `Connected`: reads the ports document of every network, then [`connected_of`].
pub(super) async fn connected_kind(entries: &[NetEntry], list: &[Device]) -> PageKind {
    let mut reads = Vec::new();
    for entry in entries {
        let doc = PortsDoc::new(entry.cfg.route.shared_replica());
        let read = timeout(DOC_READ_TIMEOUT, doc.read_all()).await;
        let views = read.ok().and_then(Result::ok).unwrap_or_default();
        reads.push((entry.id.clone(), entry.cfg.route.me(), views));
    }
    connected_of(list, reads)
}

/// Devices of every network that opened a port to that network's `me`, ordered by address.
/// `reads` holds, per network, its slot, this node's id there and its ports document.
pub(super) fn connected_of(
    list: &[Device],
    reads: Vec<(NetSlotId, PeerId, Vec<PortView>)>,
) -> PageKind {
    let issued = addresses(list);
    let mut found: Vec<(String, SharedDevice)> = Vec::new();
    for (net, me, views) in &reads {
        let mine: Vec<Device> = list.iter().filter(|d| d.net == *net).cloned().collect();
        found.extend(shared_devices(&mine, &issued, views, me));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    PageKind::Connected {
        devices: found.into_iter().map(|(_, device)| device).collect(),
    }
}

/// The devices of one network with at least one TCP port open to `me`, each with its address, ordered by it.
pub(super) fn shared_devices(
    devices: &[Device],
    issued: &HashMap<PeerId, String>,
    views: &[PortView],
    me: &PeerId,
) -> Vec<(String, SharedDevice)> {
    let shared = devices.iter().filter_map(|device| {
        let address = issued.get(&device.peer)?;
        let open = open_ports(address, device.peer, views, me);
        let name = device.name.clone();
        let online = device.online;
        (!open.is_empty()).then(|| (address.clone(), SharedDevice { name, online, open }))
    });
    let mut out: Vec<(String, SharedDevice)> = shared.collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The ports `peer` published that are open to `me`, by port number.
fn open_ports(address: &str, peer: PeerId, views: &[PortView], me: &PeerId) -> Vec<OpenPort> {
    let mut open: Vec<OpenPort> = views
        .iter()
        .filter(|v| v.author == peer && v.proto == Proto::Tcp)
        .filter_map(|v| {
            let name = v.record.as_ref()?.name_for(me)?;
            Some(OpenPort {
                port: v.port,
                name: name.to_string(),
                url: mesh_url(address, v.port),
            })
        })
        .collect();
    open.sort_by_key(|p| p.port);
    open
}

#[cfg(test)]
#[path = "connected_tests.rs"]
mod tests;
