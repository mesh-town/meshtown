//! Tests of the idle ring: the grey ring of a node that is not running, the white spinner sector of a
//! starting one, the sector count and the raster.

use super::*;
use crate::app::view::{NodeState, PageState, TrayColor, View, ICON_SIDE_PX};
use crate::app::view_ring::{role_rgb, Ring, RingRole};
use crate::app::view_ring_paint::ring_rgba_lit;
use std::f64::consts::TAU;

const SIDE: u32 = ICON_SIDE_PX;

/// The middle radius of the 36 px ring: the outer 16 px minus half the 5 px thickness.
const MIDDLE_RADIUS_PX: f64 = 13.5;

const WHITE: [u8; 3] = [255, 255, 255];

const NODES: [NodeState; 5] = [
    NodeState::Unknown,
    NodeState::Absent,
    NodeState::Starting,
    NodeState::Running,
    NodeState::Stopping,
];

mod stopped_node {
    use super::*;

    #[test]
    fn without_memory_draws_eight_grey_sectors() {
        for node in [NodeState::Absent, NodeState::Stopping] {
            let picture = after(node, 0, 0);
            assert_eq!(picture.ring, Some(grey(8)), "{node:?}");
            assert_eq!(picture.lit, None, "{node:?}");
        }
    }

    #[test]
    fn grey_raster_has_eight_grey_middles() {
        let rgba = picture_rgba(&after(NodeState::Absent, 0, 0), SIDE).expect("a ring raster");
        let middles: Vec<[u8; 3]> = (0..8).map(|k| middle_rgb(&rgba, 8, k)).collect();
        assert_eq!(middles, [role_rgb(RingRole::Offline); 8]);
    }

    #[test]
    fn absent_and_stopping_draw_the_remembered_count() {
        for node in [NodeState::Absent, NodeState::Stopping] {
            for remembered in [5, 40] {
                let picture = after(node, remembered, 3);
                assert_eq!(picture.ring, Some(grey(remembered)), "{node:?} {remembered}");
                assert_eq!(picture.lit, None, "{node:?} {remembered}: no spinner while stopped");
            }
        }
    }

    #[test]
    fn keeps_the_icon_colour() {
        let red = IconPicture { color: TrayColor::Red, ..solid() };
        let picture = red.with_idle(view_of(NodeState::Absent), IdleRing::default());
        assert_eq!(picture.color, TrayColor::Red);
    }
}

mod starting_node {
    use super::*;

    #[test]
    fn lights_the_frame_modulo_the_sector_count() {
        for (frame, lit) in [(0, 0), (2, 2), (4, 4), (5, 0), (7, 2), (u32::MAX, 0)] {
            let picture = after(NodeState::Starting, 5, frame);
            assert_eq!(picture.ring, Some(grey(5)), "frame {frame}");
            assert_eq!(picture.lit, Some(lit), "frame {frame}");
        }
    }

    #[test]
    fn without_memory_spins_eight_sectors() {
        let picture = after(NodeState::Starting, 0, 3);
        assert_eq!(picture.ring, Some(grey(8)));
        assert_eq!(picture.lit, Some(3));
    }

    #[test]
    fn clamps_the_sector_count_to_four_and_sixteen() {
        let table = [(1, 4), (2, 4), (3, 4), (4, 4), (0, 8), (16, 16), (17, 16), (40, 16)];
        for (remembered, want) in table {
            let picture = after(NodeState::Starting, remembered, 0);
            assert_eq!(picture.ring, Some(grey(want)), "remembered {remembered}");
        }
    }

    #[test]
    fn lit_sector_is_taken_modulo_the_clamped_count() {
        let picture = after(NodeState::Starting, 40, 20);
        assert_eq!(picture.ring, Some(grey(16)));
        assert_eq!(picture.lit, Some(4), "20 % 16, not 20 % 40");
    }

    #[test]
    fn same_sector_one_turn_later_is_the_same_picture() {
        assert_eq!(after(NodeState::Starting, 5, 2), after(NodeState::Starting, 5, 7));
        assert_ne!(after(NodeState::Starting, 5, 2), after(NodeState::Starting, 5, 3));
    }

    #[test]
    fn raster_matches_ring_rgba_lit() {
        let picture = after(NodeState::Starting, 5, 7);
        let want = ring_rgba_lit(&grey(5), Some(2), SIDE);
        assert_eq!(picture_rgba(&picture, SIDE), Some(want));
    }

    #[test]
    fn raster_has_exactly_the_lit_sector_white() {
        let rgba = picture_rgba(&after(NodeState::Starting, 5, 7), SIDE).expect("a ring raster");
        for k in 0..5 {
            let want = if k == 2 { WHITE } else { role_rgb(RingRole::Offline) };
            assert_eq!(middle_rgb(&rgba, 5, k), want, "sector {k}");
        }
    }
}

mod other_states {
    use super::*;

    #[test]
    fn unknown_and_running_with_an_empty_ring_stay_solid() {
        for node in [NodeState::Unknown, NodeState::Running] {
            assert_eq!(after(node, 5, 3), solid(), "{node:?}");
        }
    }

    #[test]
    fn a_ring_of_its_own_is_kept_in_every_state() {
        let own = IconPicture { ring: Some(own_ring()), ..solid() };
        for node in NODES {
            let picture = own.with_idle(view_of(node), IdleRing { remembered: 5, frame: 3 });
            assert_eq!(picture, own, "{node:?}");
        }
    }
}

mod alert {
    use super::*;

    #[test]
    fn keeps_the_grey_ring() {
        let picture = after(NodeState::Absent, 0, 0).with_alert(true);
        assert!(picture.alert);
        assert_eq!(picture.ring, Some(grey(8)));
        assert_eq!(picture.disk, None);
    }

    #[test]
    fn keeps_the_lit_sector_of_a_starting_node() {
        let picture = after(NodeState::Starting, 5, 7).with_alert(true);
        assert!(picture.alert);
        assert_eq!(picture.lit, Some(2));
    }
}

mod sector_count {
    use super::*;

    #[test]
    fn limits_are_four_eight_sixteen() {
        assert_eq!((SPIN_MIN_SECTORS, IDLE_SECTORS, SPIN_MAX_SECTORS), (4, 8, 16));
    }

    #[test]
    fn default_idle_ring_knows_nothing() {
        assert_eq!(IdleRing::default(), IdleRing { remembered: 0, frame: 0 });
    }

    #[test]
    fn unknown_memory_gives_eight_in_every_state() {
        for node in NODES {
            assert_eq!(idle_sectors(node, 0), 8, "{node:?}");
        }
    }

    #[test]
    fn stopped_keeps_the_remembered_count_unclamped() {
        for node in [NodeState::Absent, NodeState::Stopping] {
            for remembered in [1, 5, 40] {
                assert_eq!(idle_sectors(node, remembered), remembered, "{node:?}");
            }
        }
    }

    #[test]
    fn starting_is_clamped_to_four_and_sixteen() {
        let table = [(1, 4), (3, 4), (4, 4), (5, 5), (16, 16), (17, 16), (40, 16)];
        for (remembered, want) in table {
            assert_eq!(idle_sectors(NodeState::Starting, remembered), want, "remembered {remembered}");
        }
    }
}

/// A solid icon picture: no ring, no disk, no mark.
fn solid() -> IconPicture {
    IconPicture { color: TrayColor::Grey, ring: None, disk: None, alert: false, lit: None }
}

/// A ring of `n` grey (offline) sectors.
fn grey(n: u16) -> Ring {
    Ring::uniform(RingRole::Offline, n)
}

/// A ring of three members in three different roles.
fn own_ring() -> Ring {
    Ring::EMPTY.add(RingRole::ActiveExit).add(RingRole::ExitClient).add(RingRole::Online)
}

/// A solid picture after `with_idle` for `node`, with `remembered` sectors in memory and spinner frame `frame`.
fn after(node: NodeState, remembered: u16, frame: u32) -> IconPicture {
    solid().with_idle(view_of(node), IdleRing { remembered, frame })
}

/// A view in `node` state with a page that is still starting and no ring.
fn view_of(node: NodeState) -> View {
    View { node, ..View::initial() }
}

mod stuck_start {
    use super::*;

    #[test]
    fn unavailable_page_keeps_the_icon_solid() {
        let view = View { page: PageState::Unavailable, ..view_of(NodeState::Starting) };
        let picture = solid().with_idle(view, IdleRing { remembered: 5, frame: 3 });
        assert_eq!((picture.ring, picture.lit), (None, None));
    }

    #[test]
    fn spinning_follows_node_and_page() {
        let starting = view_of(NodeState::Starting);
        for page in [PageState::Starting, PageState::Ready] {
            assert!(spinning(View { page, ..starting }), "{page:?}");
        }
        assert!(!spinning(View { page: PageState::Unavailable, ..starting }));
        assert!(!spinning(view_of(NodeState::Running)));
    }
}

/// The RGB of the pixel in the middle of sector `k` of an `n`-sector 36 px ring, on the middle radius.
fn middle_rgb(rgba: &[u8], n: usize, k: usize) -> [u8; 3] {
    let angle = TAU * (k as f64 + 0.5) / n as f64;
    let half = f64::from(SIDE) / 2.0;
    let x = (half + MIDDLE_RADIUS_PX * angle.sin()).floor() as u32;
    let y = (half - MIDDLE_RADIUS_PX * angle.cos()).floor() as u32;
    let at = ((y * SIDE + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}
