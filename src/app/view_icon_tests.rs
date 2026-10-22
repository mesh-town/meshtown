//! Tests of `view`: the tray icon — `via_peer` in the facts, color choice, the "as usual" ring.

use super::*;

/// A shim with the old name: tests supply JSON, the code reads the selection.
fn facts_from_json(value: &serde_json::Value) -> Option<ColorFacts> {
    color_facts(&crate::app::snapshot::TraySnapshot::from_value(value))
}

mod facts_from_json {
    use super::*;

    mod via_peer {
        use super::*;

        #[test]
        fn route_exit_is_via_peer() {
            let state = online_with_route(Some(serde_json::json!("exit")));
            assert!(facts(&state).via_peer);
        }

        #[test]
        fn route_direct_or_missing_is_not_via_peer() {
            let direct = online_with_route(Some(serde_json::json!("direct")));
            let missing = online_with_route(None);
            let number = online_with_route(Some(serde_json::json!(5)));
            assert!(!facts(&direct).via_peer, "direct");
            assert!(!facts(&missing).via_peer, "no route");
            assert!(!facts(&number).via_peer, "state as a number");
        }
    }
}

mod tray_color {
    use super::*;

    mod in_network {
        use super::*;

        #[test]
        fn direct_is_green_ring() {
            let facts = ColorFacts {
                online: true,
                exit_available: true,
                via_peer: false,
            };
            assert_eq!(tray_color(Some(facts)), TrayColor::GreenOutline);
        }

        #[test]
        fn via_peer_is_green() {
            let facts = ColorFacts {
                online: true,
                exit_available: true,
                via_peer: true,
            };
            assert_eq!(tray_color(Some(facts)), TrayColor::Green);
        }

        #[test]
        fn exits_down_is_red() {
            let facts = ColorFacts {
                online: true,
                exit_available: false,
                via_peer: false,
            };
            assert_eq!(tray_color(Some(facts)), TrayColor::Red);
        }
    }

    mod outside_network {
        use super::*;

        #[test]
        fn offline_is_grey_even_via_peer() {
            let facts = ColorFacts {
                online: false,
                exit_available: true,
                via_peer: true,
            };
            assert_eq!(tray_color(Some(facts)), TrayColor::Grey);
        }
    }
}

mod tray_colors {
    use super::*;

    #[test]
    fn order_matches_discriminant() {
        assert_eq!(TRAY_COLORS.len(), 4);
        for c in TRAY_COLORS {
            assert_eq!(TRAY_COLORS[c as usize], c, "{c:?}");
        }
        for (i, a) in TRAY_COLORS.iter().enumerate() {
            for b in &TRAY_COLORS[i + 1..] {
                assert_ne!(a, b, "the colors in the array differ");
            }
        }
    }
}

mod icon_rgba_green_ring {
    use super::*;

    #[test]
    fn ring_length_is_square() {
        assert_eq!(icon_rgba(TrayColor::GreenOutline).len(), 36 * 36 * 4);
    }

    #[test]
    fn ring_centre_is_transparent() {
        let data = icon_rgba(TrayColor::GreenOutline);
        assert_eq!(pixel(&data, 18, 18)[3], 0, "centre");
        assert_eq!(pixel(&data, 13, 18)[3], 0, "inside the hole");
    }

    #[test]
    fn ring_edge_is_opaque_green() {
        // (18, 3) is about 14 from the centre: inside a ring of thickness 5 at the edge of radius 16
        let data = icon_rgba(TrayColor::GreenOutline);
        assert_eq!(pixel(&data, 18, 3), [60, 200, 90, 255]);
    }

    #[test]
    fn ring_outside_is_transparent() {
        let data = icon_rgba(TrayColor::GreenOutline);
        assert_eq!(pixel(&data, 0, 0)[3], 0);
    }

    #[test]
    fn filled_green_centre_is_opaque() {
        let data = icon_rgba(TrayColor::Green);
        assert_eq!(pixel(&data, 18, 18)[3], 255, "the circle didn't become a ring");
    }
}

mod icon_rgba_symmetry {
    use super::*;

    /// The alpha of `(x, y)` matches the mirrored `(35 − x, y)` and `(x, 35 − y)`: the circle sits in the centre
    /// of the raster rather than shifted by half a pixel.
    fn assert_mirrored(color: TrayColor) {
        let data = icon_rgba(color);
        let last = ICON_SIDE_PX - 1;
        for y in 0..ICON_SIDE_PX {
            for x in 0..ICON_SIDE_PX {
                let here = pixel(&data, x, y)[3];
                assert_eq!(here, pixel(&data, last - x, y)[3], "{color:?} ({x},{y}) by x");
                assert_eq!(here, pixel(&data, x, last - y)[3], "{color:?} ({x},{y}) by y");
            }
        }
    }

    #[test]
    fn green_ring_is_mirror_symmetric() {
        assert_mirrored(TrayColor::GreenOutline);
    }

    #[test]
    fn filled_green_is_mirror_symmetric() {
        assert_mirrored(TrayColor::Green);
    }
}

fn online_with_route(state: Option<serde_json::Value>) -> serde_json::Value {
    let mut value = serde_json::json!({ "node": { "status": "online" } });
    if let Some(state) = state {
        value["route"] = serde_json::json!({ "state": state, "exits": [] });
    }
    value
}

fn facts(state: &serde_json::Value) -> ColorFacts {
    facts_from_json(state).expect("node.status is a string")
}

fn pixel(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * ICON_SIDE_PX + x) * 4) as usize;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}
