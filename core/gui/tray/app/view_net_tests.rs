//! Tests of `view_net`: the reason for a grey icon when the page is ready — snapshot parsing and menu texts.

use super::*;
use crate::gui::tray::app::view::{
    node_status_menu_line, page_text, NodeState, PageState, RouteLine, TrayColor, View,
};
use crate::gui::tray::app::view_ring::Ring;
use crate::node::output::NET_NAME;
use serde_json::json;

/// A shim with the old name: tests supply JSON, the code reads the selection.
fn net_link(value: &serde_json::Value) -> NetLink {
    super::net_link(&crate::gui::tray::app::snapshot::TraySnapshot::from_value(
        value,
    ))
}

const LINKS: [NetLink; 4] = [
    NetLink::NoNetwork,
    NetLink::Connecting,
    NetLink::Lost,
    NetLink::Joined,
];

fn view(color: TrayColor, page: PageState, node: NodeState, net: NetLink) -> View {
    View {
        color,
        page,
        node,
        ring: Ring::EMPTY,
        disk: None,
        net,
    }
}

fn routes() -> [RouteLine; 4] {
    [
        RouteLine::Direct,
        RouteLine::Via("Alex".to_string()),
        RouteLine::Unavailable(Some("Alex".to_string())),
        RouteLine::Unavailable(None),
    ]
}

mod net_link {
    use super::*;

    #[test]
    fn null_network_is_no_network_for_any_status() {
        for status in ["starting", "offline", "online"] {
            let state = json!({ "node": { "status": status }, "network": null });
            assert_eq!(net_link(&state), NetLink::NoNetwork, "{status}");
        }
    }

    #[test]
    fn present_network_maps_status() {
        let cases = [
            ("starting", NetLink::Connecting),
            ("stopping", NetLink::Connecting),
            ("offline", NetLink::Lost),
            ("online", NetLink::Joined),
            ("weird", NetLink::Lost),
        ];
        for (status, expected) in cases {
            let state = json!({ "node": { "status": status }, "network": { "name": "x" } });
            assert_eq!(net_link(&state), expected, "{status}");
        }
    }

    #[test]
    fn offline_just_after_start_is_connecting() {
        let state =
            json!({ "node": { "status": "offline", "uptime_sec": 5 }, "network": { "name": "x" } });
        assert_eq!(net_link(&state), NetLink::Connecting);
        let zero = json!({ "node": { "status": "offline", "uptime_sec": 0 } });
        assert_eq!(net_link(&zero), NetLink::Connecting);
    }

    #[test]
    fn offline_long_after_start_is_lost() {
        let state = json!({ "node": { "status": "offline", "uptime_sec": 90 }, "network": { "name": "x" } });
        assert_eq!(net_link(&state), NetLink::Lost);
    }

    #[test]
    fn offline_grace_border_belongs_to_lost() {
        let at = |sec: u64| json!({ "node": { "status": "offline", "uptime_sec": sec } });
        assert_eq!(net_link(&at(59)), NetLink::Connecting);
        assert_eq!(net_link(&at(60)), NetLink::Lost);
        assert_eq!(net_link(&at(61)), NetLink::Lost);
    }

    #[test]
    fn offline_without_uptime_is_lost() {
        for uptime in [json!(null), json!("5"), json!(-1)] {
            let state = json!({ "node": { "status": "offline", "uptime_sec": uptime } });
            assert_eq!(net_link(&state), NetLink::Lost, "{uptime}");
        }
    }

    #[test]
    fn uptime_does_not_change_other_statuses() {
        let cases = [
            ("online", 5, NetLink::Joined),
            ("online", 90, NetLink::Joined),
            ("starting", 5, NetLink::Connecting),
            ("starting", 90, NetLink::Connecting),
            ("stopping", 90, NetLink::Connecting),
            ("weird", 5, NetLink::Lost),
        ];
        for (status, uptime, expected) in cases {
            let state = json!({ "node": { "status": status, "uptime_sec": uptime } });
            assert_eq!(net_link(&state), expected, "{status} {uptime}");
        }
    }

    #[test]
    fn no_network_wins_over_young_offline() {
        let state = json!({ "node": { "status": "offline", "uptime_sec": 5 }, "network": null });
        assert_eq!(net_link(&state), NetLink::NoNetwork);
    }

    #[test]
    fn missing_network_key_counts_as_present() {
        let state = json!({ "node": { "status": "offline" } });
        assert_eq!(net_link(&state), NetLink::Lost);
    }

    #[test]
    fn missing_node_is_no_network() {
        assert_eq!(
            net_link(&json!({ "network": { "name": "x" } })),
            NetLink::NoNetwork
        );
        assert_eq!(net_link(&json!({})), NetLink::NoNetwork);
    }
}

mod grey_texts {
    use super::*;

    fn grey_ready(net: NetLink) -> View {
        view(TrayColor::Grey, PageState::Ready, NodeState::Running, net)
    }

    #[test]
    fn status_line_follows_the_table() {
        let connecting = format!("Connecting to the {NET_NAME} network…");
        let cases = [
            (
                NetLink::NoNetwork,
                format!("You haven't joined a {NET_NAME} network yet"),
            ),
            (NetLink::Connecting, connecting.clone()),
            (
                NetLink::Lost,
                format!("No connection to the {NET_NAME} network"),
            ),
            (NetLink::Joined, connecting),
        ];
        for route in routes() {
            for (net, expected) in &cases {
                assert_eq!(
                    &node_status_menu_line(grey_ready(*net), &route),
                    expected,
                    "{net:?}"
                );
            }
        }
    }

    #[test]
    fn open_text_follows_the_table() {
        for net in LINKS {
            let expected = match net {
                NetLink::NoNetwork => format!("Join a {NET_NAME} network…"),
                _ => "Open Mesh Town…".to_string(),
            };
            assert_eq!(page_text(grey_ready(net)), expected, "{net:?}");
        }
    }

    #[test]
    fn other_views_ignore_the_link() {
        let nodes = [
            NodeState::Absent,
            NodeState::Starting,
            NodeState::Stopping,
            NodeState::Unknown,
            NodeState::Running,
        ];
        let colors = [
            TrayColor::Grey,
            TrayColor::Green,
            TrayColor::GreenOutline,
            TrayColor::Red,
        ];
        let pages = [
            PageState::Ready,
            PageState::Starting,
            PageState::Unavailable,
            PageState::Disabled,
            PageState::PortBusy(7070),
        ];
        for route in routes() {
            for node in nodes {
                for color in colors {
                    for page in pages {
                        check_same_for_all_links(color, page, node, &route);
                    }
                }
            }
        }
    }

    fn check_same_for_all_links(c: TrayColor, o: PageState, n: NodeState, r: &RouteLine) {
        let grey_ready_running =
            c == TrayColor::Grey && o == PageState::Ready && n == NodeState::Running;
        if grey_ready_running {
            return;
        }
        let base = view(c, o, n, NetLink::NoNetwork);
        for net in LINKS {
            let v = view(c, o, n, net);
            assert_eq!(
                node_status_menu_line(v, r),
                node_status_menu_line(base, r),
                "{v:?}"
            );
            assert_eq!(page_text(v), page_text(base), "{v:?}");
        }
    }
}
