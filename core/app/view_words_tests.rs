//! Tests of `view`: the tray words — the second menu item (`page_text`) and the first line (`node_status_menu_line`),
//! the texts for each combination.

use super::*;

mod open_text {
    use super::*;

    mod page_ready {
        use super::*;

        #[test]
        fn in_network_opens_app() {
            // given / when / then — for every "in a network" color
            for color in [TrayColor::GreenOutline, TrayColor::Green, TrayColor::Red] {
                let view = view(NodeState::Running, color, PageState::Ready);
                assert_eq!(page_text(view), "Open Mesh Town…", "{color:?}");
            }
        }

        #[test]
        fn outside_network_offers_join() {
            let view = view(NodeState::Running, TrayColor::Grey, PageState::Ready);
            assert_eq!(page_text(view), "Join a Mesh Town network…");
        }

        #[test]
        fn grey_but_node_not_running_opens_app() {
            // joining is offered only to a running node
            let view = view(NodeState::Absent, TrayColor::Grey, PageState::Ready);
            assert_eq!(page_text(view), "Open Mesh Town…");
        }
    }

    mod page_not_ready {
        use super::*;

        #[test]
        fn other_open_states_keep_old_texts() {
            let cases = [
                (PageState::Starting, "Mesh Town page is starting…"),
                (
                    PageState::Disabled,
                    "Turn the page on in the settings file (web.enabled), then restart",
                ),
                (
                    PageState::PortBusy(8080),
                    "Close that program — \"Start Mesh Town\" will become available",
                ),
                (PageState::Unavailable, "Mesh Town page is unavailable"),
            ];
            for (page, expected) in cases {
                let view = view(NodeState::Running, TrayColor::Grey, page);
                assert_eq!(page_text(view), expected, "{page:?}");
            }
        }
    }
}

mod status_line {
    use super::*;

    mod running_in_network {
        use super::*;

        #[test]
        fn via_label_with_country_is_shown_as_is() {
            let view = view(NodeState::Running, TrayColor::Green, PageState::Ready);
            let route = RouteLine::Via("peer-one · DE".to_string());
            assert_eq!(
                node_status_menu_line(view, &route),
                "In the browser, sites open via peer-one · DE"
            );
        }

        #[test]
        fn single_unavailable_has_red_mark_and_consequence() {
            let view = view(NodeState::Running, TrayColor::Red, PageState::Ready);
            let route = RouteLine::Unavailable(Some("peer-one · DE".to_string()));
            assert_eq!(
                node_status_menu_line(view, &route),
                "🔴 peer-one · DE is offline — sites won't open until it's back"
            );
        }

        #[test]
        fn no_route_line_starts_with_node_works() {
            let routes = [
                RouteLine::Direct,
                RouteLine::Via("peer-one".to_string()),
                RouteLine::Unavailable(Some("peer-one".to_string())),
                RouteLine::Unavailable(None),
            ];
            for color in [TrayColor::Green, TrayColor::GreenOutline, TrayColor::Red] {
                for route in &routes {
                    let view = view(NodeState::Running, color, PageState::Ready);
                    let text = node_status_menu_line(view, route);
                    assert!(
                        !text.starts_with("Node is running"),
                        "{color:?} {route:?}: {text:?}"
                    );
                }
            }
        }
    }

    mod outside_network {
        use super::*;

        #[test]
        fn outside_network_ignores_route() {
            let view = view(NodeState::Running, TrayColor::Grey, PageState::Ready);
            let route = RouteLine::Via("peer-one".to_string());
            assert_eq!(node_status_menu_line(view, &route), "You haven't joined a Mesh Town network yet");
        }
    }
}

fn view(node: NodeState, color: TrayColor, page: PageState) -> View {
    View {
        color,
        page,
        node,
        ring: Ring::EMPTY,
        disk: None,
        net: NetLink::NoNetwork,
    }
}
