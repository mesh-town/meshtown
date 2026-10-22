//! The `ports` section of the demo node's snapshot: its own publications come from the settings (after a PUT `/api/config`
//! `apply_config` puts the new list there), while "Network ports" is one fixture: the first demo member
//! publishes port 9000. A port opened here (`ports.opened`) is shown as listening.

use meshtown_core::web_gui::{NetPort, OpenedInfo, OwnPort, PortsInfo};

use crate::node::DemoState;

const FIXTURE_PORT: u16 = 9000;
const FIXTURE_NAME: &str = "demo-web";

fn own_rows(st: &DemoState) -> Vec<OwnPort> {
    st.settings
        .net
        .ports_published
        .iter()
        .map(|p| OwnPort {
            port: p.port,
            name: p.name.clone(),
            local: "unknown".to_string(),
        })
        .collect()
}

fn opened_info(st: &DemoState, node: &str, port: u16) -> Option<OpenedInfo> {
    st.settings
        .net
        .ports_opened
        .iter()
        .find(|o| o.node.to_string() == node && o.port == port)
        .map(|o| OpenedInfo {
            local: o.local,
            state: "listening".to_string(),
            error: None,
        })
}

fn fixture_rows(st: &DemoState) -> Vec<NetPort> {
    st.peers
        .first()
        .map(|p| {
            let node = p.id_full.to_string();
            NetPort {
                opened: opened_info(st, &node, FIXTURE_PORT),
                node,
                node_name: p.name.clone(),
                online: p.online,
                port: FIXTURE_PORT,
                name: FIXTURE_NAME.to_string(),
                published: true,
            }
        })
        .into_iter()
        .collect()
}

pub(crate) fn demo_ports(st: &DemoState) -> PortsInfo {
    PortsInfo {
        published: own_rows(st),
        network: fixture_rows(st),
    }
}
