//! Tests of the ring model: counting in `Ring`, equality, the choice of a solid icon (`ring_to_draw`).

use super::*;
use crate::gui::tray::app::view::{NetLink, NodeState, PageState, TrayColor};

const COLORS: [TrayColor; 4] = [
    TrayColor::Grey,
    TrayColor::Red,
    TrayColor::Green,
    TrayColor::GreenOutline,
];

const ALL_ROLES: [RingRole; 5] = [
    RingRole::ExitDown,
    RingRole::ActiveExit,
    RingRole::ExitClient,
    RingRole::Online,
    RingRole::Offline,
];

fn ring_of(roles: &[RingRole]) -> Ring {
    roles.iter().fold(Ring::EMPTY, |ring, &role| ring.add(role))
}

/// Roles in sector order.
fn roles_of(ring: Ring) -> Vec<RingRole> {
    (0..ring.len()).map(|k| ring.role_at(k)).collect()
}

fn view_with(color: TrayColor, ring: Ring) -> View {
    View {
        color,
        page: PageState::Ready,
        node: NodeState::Running,
        ring,
        disk: None,
        net: NetLink::NoNetwork,
    }
}

mod role_colours {
    use super::*;

    #[test]
    fn exit_down_is_the_solid_red_icon_colour() {
        let (r, g, b) = (225, 60, 50);
        assert_eq!(role_rgb(RingRole::ExitDown), [r, g, b]);
        let icon = crate::gui::tray::app::view::icon_rgba(TrayColor::Red);
        let centre = ((18 * crate::gui::tray::app::view::ICON_SIDE_PX + 18) * 4) as usize;
        assert_eq!(icon[centre..centre + 3], [r, g, b]);
    }

    #[test]
    fn exit_down_is_the_first_sector() {
        let ring = ring_of(&[RingRole::Offline, RingRole::ExitDown, RingRole::ActiveExit]);
        assert_eq!(
            roles_of(ring),
            [RingRole::ExitDown, RingRole::ActiveExit, RingRole::Offline]
        );
        assert_eq!(RING_ROLES.len(), RING_ROLE_COUNT);
    }

    #[test]
    fn exit_via_is_light_blue() {
        assert_eq!(role_rgb(RingRole::ActiveExit), [90, 170, 255]);
    }

    #[test]
    fn five_roles_have_distinct_colours() {
        let colours: Vec<[u8; 3]> = ALL_ROLES.iter().map(|&role| role_rgb(role)).collect();
        for (i, a) in colours.iter().enumerate() {
            assert!(colours[i + 1..].iter().all(|b| a != b), "{colours:?}");
        }
    }
}

mod ring_counts {
    use super::*;

    #[test]
    fn roles_come_out_in_rank_order() {
        let ring = ring_of(&[
            RingRole::Offline,
            RingRole::Online,
            RingRole::ExitClient,
            RingRole::ActiveExit,
            RingRole::ExitDown,
        ]);
        assert_eq!(roles_of(ring), ALL_ROLES);
    }

    #[test]
    fn equal_neighbours_form_an_arc() {
        let ring = ring_of(&[RingRole::Online, RingRole::ActiveExit, RingRole::Online]);
        assert_eq!(
            roles_of(ring),
            [RingRole::ActiveExit, RingRole::Online, RingRole::Online]
        );
    }

    #[test]
    fn forty_peers_are_all_kept() {
        let mut roles = vec![RingRole::ExitDown];
        roles.push(RingRole::ActiveExit);
        roles.extend([RingRole::ExitClient; 3]);
        roles.extend([RingRole::Online; 20]);
        roles.extend([RingRole::Offline; 15]);
        let ring = ring_of(&roles);
        assert_eq!(ring.len(), 40);
        let counts = ALL_ROLES.map(|role| ring.count(role));
        assert_eq!(counts, [1, 1, 3, 20, 15]);
    }

    #[test]
    fn counter_saturates() {
        let full = (0..u16::MAX).fold(Ring::EMPTY, |ring, _| ring.add(RingRole::Online));
        assert_eq!(full.count(RingRole::Online), u16::MAX);
        assert_eq!(full.add(RingRole::Online).count(RingRole::Online), u16::MAX);
    }

    #[test]
    fn role_past_the_end_is_offline() {
        let ring = ring_of(&[RingRole::Online]);
        assert_eq!(ring.role_at(0), RingRole::Online);
        assert_eq!(ring.role_at(1), RingRole::Offline);
        assert_eq!(ring.role_at(1000), RingRole::Offline);
        assert_eq!(Ring::EMPTY.role_at(0), RingRole::Offline);
    }

    #[test]
    fn empty_ring_is_empty() {
        assert!(Ring::EMPTY.is_empty());
        assert_eq!(Ring::EMPTY.len(), 0);
        assert_eq!(ring_of(&[]), Ring::EMPTY);
        assert!(!ring_of(&[RingRole::Offline]).is_empty());
    }
}

mod ring_equality {
    use super::*;

    #[test]
    fn same_roles_in_any_add_order_are_equal() {
        let a = ring_of(&[RingRole::Online, RingRole::Offline, RingRole::ActiveExit]);
        let b = ring_of(&[RingRole::ActiveExit, RingRole::Offline, RingRole::Online]);
        assert_eq!(a, b);
    }

    #[test]
    fn different_composition_differs() {
        let base = ring_of(&[RingRole::Online, RingRole::Online]);
        assert_ne!(base, ring_of(&[RingRole::Online]));
        assert_ne!(base, ring_of(&[RingRole::Online, RingRole::Offline]));
        assert_ne!(base, Ring::EMPTY);
    }

    #[test]
    fn view_with_different_ring_is_not_equal() {
        let one = view_with(TrayColor::Green, ring_of(&[RingRole::Online]));
        let two = view_with(
            TrayColor::Green,
            ring_of(&[RingRole::Online, RingRole::Online]),
        );
        assert_ne!(one, two);
        assert_eq!(
            one,
            view_with(TrayColor::Green, ring_of(&[RingRole::Online]))
        );
    }
}

mod icon_ring_choice {
    use super::*;

    #[test]
    fn every_colour_with_empty_ring_stays_solid() {
        for color in COLORS {
            assert_eq!(
                ring_to_draw(view_with(color, Ring::EMPTY)),
                None,
                "{color:?}"
            );
        }
    }

    #[test]
    fn every_colour_with_ring_gets_sectors() {
        let ring = ring_of(&ALL_ROLES);
        for color in COLORS {
            assert_eq!(
                ring_to_draw(view_with(color, ring)),
                Some(ring),
                "{color:?}"
            );
        }
    }
}

mod uniform_ring {
    use super::*;

    #[test]
    fn has_count_sectors_of_one_role() {
        let ring = Ring::uniform(RingRole::Offline, 5);
        assert_eq!(ring.len(), 5);
        assert_eq!(roles_of(ring), [RingRole::Offline; 5]);
        assert_eq!(ring.count(RingRole::Offline), 5);
    }

    #[test]
    fn keeps_the_requested_role() {
        for role in RING_ROLES {
            let ring = Ring::uniform(role, 3);
            assert_eq!(roles_of(ring), [role; 3], "{role:?}");
        }
    }

    #[test]
    fn equals_the_ring_built_by_add() {
        let by_add = ring_of(&[RingRole::Online; 7]);
        assert_eq!(Ring::uniform(RingRole::Online, 7), by_add);
        assert_ne!(Ring::uniform(RingRole::Offline, 7), by_add);
    }

    #[test]
    fn zero_count_is_the_empty_ring() {
        for role in RING_ROLES {
            assert_eq!(Ring::uniform(role, 0), Ring::EMPTY, "{role:?}");
        }
        assert!(Ring::uniform(RingRole::Offline, 0).is_empty());
    }
}
