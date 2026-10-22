//! Tests of `view`: the tray status line / tooltip — `node_status_menu_line` by `NodeState`, `page`, color and route.

use super::*;
use crate::node::output::NET_NAME;

// `tooltip()` was removed (the tooltip = `node_status_menu_line`) — its replacement is below, `mod status_line`.

/// The tray status line / tooltip: the text depends on `NodeState`, and for `Running`
/// also on `page`/`color`/the route.
mod status_line {
    use super::*;

    #[test]
    fn absent_is_stopped() {
        let view = View {
            color: TrayColor::Grey,
            page: PageState::Unavailable,
            node: NodeState::Absent,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "Mesh Town is stopped — sites won't open through it"
        );
    }

    #[test]
    fn starting_is_launching() {
        let view = View {
            color: TrayColor::Grey,
            page: PageState::Starting,
            node: NodeState::Starting,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "Mesh Town is starting…"
        );
    }

    #[test]
    fn stopping_is_stopping() {
        let view = View {
            color: TrayColor::Grey,
            page: PageState::Unavailable,
            node: NodeState::Stopping,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "Mesh Town is stopping…"
        );
    }

    #[test]
    fn unknown_is_not_responding() {
        // given: the initial view before the first poll (View::initial())
        assert_eq!(
            node_status_menu_line(View::initial(), &RouteLine::Direct),
            "Mesh Town is not responding"
        );
    }

    #[test]
    fn unknown_with_port_busy_names_the_port() {
        let view = View {
            color: TrayColor::Grey,
            page: PageState::PortBusy(7777),
            node: NodeState::Unknown,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "Mesh Town isn't running: another program uses page port 7777"
        );
    }

    #[test]
    fn unknown_with_page_disabled_says_icon_does_not_see_node() {
        let view = View {
            color: TrayColor::Grey,
            page: PageState::Disabled,
            node: NodeState::Unknown,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "The icon can't reach Mesh Town: page is off in settings file"
        );
    }

    #[test]
    fn unknown_initial_view_stays_not_responding() {
        let view = View::initial();
        assert_eq!(view.node, NodeState::Unknown);
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "Mesh Town is not responding"
        );
    }

    #[test]
    fn running_with_page_unavailable_is_not_responding() {
        // given: our own node is running, but HTTP on its page is silent (RUNNING_UNAVAILABLE_VIEW)
        let view = View {
            color: TrayColor::Grey,
            page: PageState::Unavailable,
            node: NodeState::Running,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "Mesh Town is not responding"
        );
    }

    #[test]
    fn running_grey_ready_is_offline() {
        // given: the page answered (Ready), but facts_from_json gave grey — there is no network (`network: null`)
        let view = View {
            color: TrayColor::Grey,
            page: PageState::Ready,
            node: NodeState::Running,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            format!("You haven't joined a {NET_NAME} network yet")
        );
    }

    fn running_green() -> View {
        View {
            color: TrayColor::Green,
            page: PageState::Ready,
            node: NodeState::Running,
            ring: Ring::EMPTY,
            disk: None,
            net: NetLink::NoNetwork,
        }
    }

    #[test]
    fn running_green_direct_route() {
        let view = View {
            color: TrayColor::GreenOutline,
            ..running_green()
        };
        assert_eq!(
            node_status_menu_line(view, &RouteLine::Direct),
            "In the browser, sites open directly"
        );
    }

    #[test]
    fn running_green_via_named_exit() {
        let route = RouteLine::Via("Alex".to_string());
        assert_eq!(
            node_status_menu_line(running_green(), &route),
            "In the browser, sites open via Alex"
        );
    }

    #[test]
    fn running_green_single_exit_unavailable() {
        let route = RouteLine::Unavailable(Some("Alex".to_string()));
        assert_eq!(
            node_status_menu_line(running_green(), &route),
            "🔴 Alex is offline — sites won't open until it's back"
        );
    }

    #[test]
    fn running_green_several_exits_unavailable() {
        let route = RouteLine::Unavailable(None);
        assert_eq!(
            node_status_menu_line(running_green(), &route),
            "Chosen members are offline — sites won't open"
        );
    }

    /// A bare "network" is forbidden: every "network" in the tray texts follows {NET_NAME} ("the Mesh Town network").
    /// The check swaps the pair so the name follows the word, then looks at what follows each "network".
    #[test]
    fn network_word_always_names_the_network() {
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
        ];
        let links = [
            NetLink::NoNetwork,
            NetLink::Connecting,
            NetLink::Lost,
            NetLink::Joined,
        ];
        for route in &routes() {
            for node in nodes {
                for color in colors {
                    for page in pages {
                        for net in links {
                            let view = View {
                                color,
                                page,
                                node,
                                ring: Ring::EMPTY,
                                disk: None,
                                net,
                            };
                            assert_names_network(&node_status_menu_line(view, route));
                            assert_names_network(&page_text(view));
                        }
                    }
                }
            }
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

    fn assert_names_network(text: &str) {
        let text = text.replace(
            &format!("{NET_NAME} network"),
            &format!("network {NET_NAME}"),
        );
        for word in ["network"] {
            for (at, _) in text.match_indices(word) {
                let rest = &text[at + word.len()..];
                assert!(
                    rest.starts_with(&format!(" {NET_NAME}")),
                    "the tray text named our network without NET_NAME: {text:?}"
                );
            }
        }
    }
}
