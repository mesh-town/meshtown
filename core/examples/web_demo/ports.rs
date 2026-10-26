//! The `ports` section of the demo node's snapshot: its own publications come from the settings (after a PUT `/api/config`
//! `apply_config` puts the new list there), while "Network ports" is one fixture: the first demo member
//! publishes TCP port 9000 and a UDP port 5353. A port opened here (`ports.opened`) is shown as listening.

use meshtown_core::ports_sharing::Proto;
use meshtown_core::web_gui::{NetPort, OpenedInfo, OwnPort, PortsInfo};

use crate::node::DemoState;

const FIXTURE_PORT: u16 = 9000;
const FIXTURE_NAME: &str = "demo-web";
const FIXTURE_UDP_PORT: u16 = 5353;
const FIXTURE_UDP_NAME: &str = "demo-dns";

fn own_rows(st: &DemoState) -> Vec<OwnPort> {
    st.settings
        .net
        .ports_published
        .iter()
        .map(|p| OwnPort {
            port: p.port,
            proto: p.proto,
            name: p.name.clone(),
            local: "unknown".to_string(),
        })
        .collect()
}

fn opened_info(st: &DemoState, node: &str, proto: Proto, port: u16) -> Option<OpenedInfo> {
    st.settings
        .net
        .ports_opened
        .iter()
        .find(|o| o.node.to_string() == node && o.proto == proto && o.port == port)
        .map(|o| OpenedInfo {
            local: o.local,
            state: "listening".to_string(),
            error: None,
            last_error: None,
        })
}

fn fixture_row(st: &DemoState, proto: Proto, port: u16, name: &str) -> Option<NetPort> {
    let p = st.peers.first()?;
    let node = p.id_full.to_string();
    let web_url = match (proto, p.name.as_ref()) {
        (Proto::Tcp, Some(n)) => Some(format!("http://{n}.meshtown:{port}")),
        _ => None,
    };
    Some(NetPort {
        opened: opened_info(st, &node, proto, port),
        node,
        node_name: p.name.clone(),
        online: p.online,
        port,
        proto,
        name: name.to_string(),
        published: true,
        web_url,
    })
}

fn fixture_rows(st: &DemoState) -> Vec<NetPort> {
    [
        fixture_row(st, Proto::Tcp, FIXTURE_PORT, FIXTURE_NAME),
        fixture_row(st, Proto::Udp, FIXTURE_UDP_PORT, FIXTURE_UDP_NAME),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(crate) fn demo_ports(st: &DemoState) -> PortsInfo {
    PortsInfo {
        published: own_rows(st),
        network: fixture_rows(st),
    }
}
