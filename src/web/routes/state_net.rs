//! Network keys of `/api/state`: the list of networks, the router, the network of this request, `web`, `restart_pending`,
//! the PAC address for the device that opened the page. Added after `apply_state_extras`.

use std::net::SocketAddr;

use serde_json::{json, Value};

use crate::config_file::{IfaceFilter, ListenAccess};
use crate::web::auth::iface_gate::{judge, IfaceView};
use crate::web::auth::iface_kind::is_router;
use crate::web::auth::iface_nets::{networks, NetEntry};
use crate::web::auth::state::AuthState;
use crate::web::http::ConnInfo;
use crate::web::{NodeSnapshot, SocksLan};

#[derive(serde::Serialize)]
pub(super) struct WebState {
    access: &'static str,
    listening: &'static str,
    ifaces: Option<Vec<String>>,
}

pub(super) struct NetState {
    router: bool,
    via_iface: Option<String>,
    networks: Vec<NetEntry>,
    side: &'static str,
    web: WebState,
    restart_pending: Vec<&'static str>,
    pac_url: Option<String>,
}

/// The side the network list judges, and that side's filter.
fn side_of(snap: &NodeSnapshot, auth: &AuthState) -> (&'static str, ListenAccess, IfaceFilter) {
    if snap.socks.enabled && snap.socks.access == ListenAccess::Localnet.as_str() {
        return ("socks", ListenAccess::Localnet, IfaceFilter::from_state(snap.socks.ifaces.clone()));
    }
    if auth.mode() == ListenAccess::Localnet {
        return ("web", ListenAccess::Localnet, auth.ifaces());
    }
    ("none", ListenAccess::Localhost, IfaceFilter::Auto)
}

/// The network of this request; `None` — loopback, the machine's own address, no connection.
fn via_of(view: &IfaceView, conn: Option<&ConnInfo>) -> Option<String> {
    let c = conn?;
    let v = judge(ListenAccess::Localnet, &IfaceFilter::Any, view, c.client_addr.ip(), Some(c.local.ip()));
    v.iface
}

pub(super) fn net_state(
    snap: &NodeSnapshot,
    auth: &AuthState,
    view: &IfaceView,
    conn: Option<&ConnInfo>,
    restart_pending: Vec<&'static str>,
) -> NetState {
    let (side, mode, filter) = side_of(snap, auth);
    let via_iface = via_of(view, conn);
    let pac_url = pac_for(snap.socks.pac_url.as_deref(), &snap.socks.access, via_iface.as_deref(), &snap.socks.lan);
    NetState {
        router: is_router(view),
        via_iface,
        networks: networks(mode, &filter, view),
        side,
        web: WebState {
            access: auth.access().as_str(),
            listening: auth.bound_access().as_str(),
            ifaces: auth.ifaces().to_state(),
        },
        restart_pending,
        pac_url,
    }
}

/// The PAC address for a device: from the network under `localnet` — the proxy address in its network (otherwise the first
/// open one); in other cases — the snapshot address as is.
pub(super) fn pac_for(
    snap_pac: Option<&str>,
    access: &str,
    via: Option<&str>,
    lan: &[SocksLan],
) -> Option<String> {
    let keep = || snap_pac.map(str::to_string);
    if access != ListenAccess::Localnet.as_str() {
        return keep();
    }
    let Some(via) = via else { return keep() };
    let pick = lan.iter().find(|l| l.iface == via).or(lan.first());
    match pick.and_then(|l| l.addr.parse::<SocketAddr>().ok()) {
        Some(a) => Some(crate::proxy::pac_url(a)),
        None => keep(),
    }
}

/// Inserts the network keys: into `node` — `router`, `via_iface`, `networks`, `networks_side`; into
/// the root — `web`, `restart_pending`; `socks.pac_url` is replaced only if `socks` exists.
pub(super) fn apply_net(value: &mut Value, net: NetState) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    obj.insert("web".into(), json!(net.web));
    obj.insert("restart_pending".into(), json!(net.restart_pending));
    if let Some(node) = obj.get_mut("node").and_then(Value::as_object_mut) {
        node.insert("router".into(), json!(net.router));
        node.insert("via_iface".into(), json!(net.via_iface));
        node.insert("networks".into(), json!(net.networks));
        node.insert("networks_side".into(), json!(net.side));
    }
    if let Some(socks) = obj.get_mut("socks").and_then(Value::as_object_mut) {
        socks.insert("pac_url".into(), json!(net.pac_url));
    }
}

#[cfg(test)]
#[path = "state_net_tests.rs"]
mod tests;
