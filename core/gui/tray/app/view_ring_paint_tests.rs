//! Tests of the ring-of-sectors raster: colors, thickness, gaps, symmetry, sizes 36/22/16.

use super::*;
use crate::gui::tray::app::view::{icon_rgba, TrayColor, View, ICON_SIDE_PX};
use crate::gui::tray::app::view_ring::{ring_to_draw, RingRole, RING_ROLES};
use std::f64::consts::{PI, TAU};

const ALL_ROLES: [RingRole; 5] = [
    RingRole::ExitDown,
    RingRole::ActiveExit,
    RingRole::ExitClient,
    RingRole::Online,
    RingRole::Offline,
];
const SIDES: [u32; 3] = [36, 22, 16];

/// Rings of 1, 2, 5 and 8 sectors: the 5-sector one has one role each, the 8-sector one has two each.
const CASES: [(usize, &[RingRole]); 4] = [
    (1, &[RingRole::Online]),
    (2, &[RingRole::ActiveExit, RingRole::Online]),
    (5, &ALL_ROLES),
    (
        8,
        &[
            RingRole::ActiveExit,
            RingRole::ActiveExit,
            RingRole::ExitClient,
            RingRole::ExitClient,
            RingRole::Online,
            RingRole::Online,
            RingRole::Offline,
            RingRole::Offline,
        ],
    ),
];

fn ring_of(roles: &[RingRole]) -> Ring {
    roles.iter().fold(Ring::EMPTY, |ring, &role| ring.add(role))
}

fn same_role(n: usize) -> Ring {
    ring_of(&vec![RingRole::Online; n])
}

/// The middle radius of the ring: the outer `side·16/36` minus half the thickness.
fn rm(side: u32) -> f64 {
    f64::from(side) * 16.0 / 36.0 - f64::from(ring_spec(side).thickness_px) / 2.0
}

fn pixel(data: &[u8], side: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * side + x) * 4) as usize;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

/// A pixel at angle `angle` (radians from 12 o'clock clockwise) and distance `r` from the centre `side/2`.
fn pixel_at(data: &[u8], side: u32, angle: f64, r: f64) -> [u8; 4] {
    let half = f64::from(side) / 2.0;
    let x = (half + r * angle.sin()).floor() as u32;
    let y = (half - r * angle.cos()).floor() as u32;
    pixel(data, side, x, y)
}

mod spec {
    use super::*;

    #[test]
    fn table_by_side() {
        let big = ring_spec(36);
        assert_eq!((big.thickness_px, big.gap_px), (5, 1));
        assert_eq!((ring_spec(40).thickness_px, ring_spec(40).gap_px), (5, 1));
        for side in [22, 16] {
            let small = ring_spec(side);
            assert_eq!((small.thickness_px, small.gap_px), (3, 0), "{side}");
        }
    }
}

mod paint {
    use super::*;

    #[test]
    fn length_is_square_rgba() {
        for side in SIDES {
            let data = ring_rgba(&ring_of(&ALL_ROLES), side);
            assert_eq!(data.len(), (side * side * 4) as usize, "{side}");
        }
    }

    #[test]
    fn sector_middles_have_role_colour_and_are_opaque() {
        for side in SIDES {
            for (n, roles) in CASES {
                let ring = ring_of(roles);
                let data = ring_rgba(&ring, side);
                for k in 0..n {
                    let angle = TAU * (k as f64 + 0.5) / n as f64;
                    let [r, g, b, a] = pixel_at(&data, side, angle, rm(side));
                    let want = if side == 36 { 255 } else { 200 };
                    assert_eq!([r, g, b], role_rgb(ring.role_at(k)), "{side} n={n} k={k}");
                    assert!(a >= want, "{side} n={n} k={k}: alpha {a}");
                }
            }
        }
    }

    #[test]
    fn thickness_along_the_row_matches_spec() {
        for side in SIDES {
            let data = ring_rgba(&same_role(1), side);
            let y = side / 2;
            let solid = (side / 2..side)
                .filter(|&x| pixel(&data, side, x, y)[3] >= 128)
                .count();
            assert_eq!(solid as u32, ring_spec(side).thickness_px, "{side}");
        }
    }

    #[test]
    fn centre_and_corner_are_transparent() {
        for side in SIDES {
            let data = ring_rgba(&ring_of(&ALL_ROLES), side);
            assert_eq!(
                pixel(&data, side, side / 2, side / 2)[3],
                0,
                "centre {side}"
            );
            assert_eq!(pixel(&data, side, 0, 0)[3], 0, "corner {side}");
        }
    }

    #[test]
    fn empty_ring_is_fully_transparent() {
        for side in SIDES {
            let data = ring_rgba(&Ring::EMPTY, side);
            assert_eq!(data.len(), (side * side * 4) as usize);
            assert!(data.iter().all(|&byte| byte == 0), "{side}");
        }
    }

    #[test]
    fn single_sector_is_full_ring_without_gap() {
        let data = ring_rgba(&same_role(1), 36);
        assert_eq!(pixel_at(&data, 36, 0.0, rm(36))[3], 255);
        assert_eq!(pixel_at(&data, 36, PI, rm(36))[3], 255);
    }

    #[test]
    fn sectors_differ_from_solid_circle() {
        let ring = ring_of(&ALL_ROLES);
        assert_ne!(ring_rgba(&ring, ICON_SIDE_PX), icon_rgba(TrayColor::Green));
        assert_ne!(
            ring_rgba(&ring, ICON_SIDE_PX),
            icon_rgba(TrayColor::GreenOutline)
        );
        assert_eq!(ring_to_draw(View::initial()), None);
    }
}

mod gaps {
    use super::*;

    #[test]
    fn same_role_neighbours_are_split_by_a_gap() {
        for n in [2usize, 4, 8] {
            let data = ring_rgba(&same_role(n), 36);
            for j in 0..n {
                let border = pixel_at(&data, 36, TAU * j as f64 / n as f64, rm(36))[3];
                let middle = pixel_at(&data, 36, TAU * (j as f64 + 0.5) / n as f64, rm(36))[3];
                assert!(border <= 160, "n={n} border {j}: alpha {border}");
                assert_eq!(middle, 255, "n={n} middle {j}");
            }
        }
    }

    #[test]
    fn different_roles_blend_without_gap() {
        let ring = ring_of(&[RingRole::ActiveExit, RingRole::ExitClient, RingRole::Online]);
        let data = ring_rgba(&ring, 36);
        let [r, _, _, a] = pixel_at(&data, 36, TAU / 3.0, rm(36));
        assert_eq!(a, 255);
        assert!(r > 40 && r < 175, "red {r}: blending, not a step");
    }

    #[test]
    fn arc_threshold_is_between_16_and_17_peers() {
        let alpha = |n| pixel_at(&ring_rgba(&same_role(n), 36), 36, 0.0, rm(36))[3];
        assert!(alpha(16) <= 160, "16: there is a gap");
        assert_eq!(alpha(17), 255, "17: no gap");
    }

    #[test]
    fn small_sizes_have_no_gap() {
        for side in [22, 16] {
            let data = ring_rgba(&same_role(4), side);
            for j in 0..4 {
                let alpha = pixel_at(&data, side, TAU * f64::from(j) / 4.0, rm(side))[3];
                assert!(alpha >= 200, "{side} border {j}: alpha {alpha}");
            }
        }
    }
}

mod symmetry {
    use super::*;

    fn assert_mirrored(data: &[u8], side: u32, what: &str) {
        for y in 0..side {
            for x in 0..side {
                let here = pixel(data, side, x, y);
                for other in [
                    pixel(data, side, side - 1 - x, y),
                    pixel(data, side, x, side - 1 - y),
                ] {
                    assert!(
                        here[3].abs_diff(other[3]) <= 1,
                        "{what} ({x},{y}): {here:?} {other:?}"
                    );
                    assert!(
                        here[3] == 0 || here[..3] == other[..3],
                        "{what} ({x},{y}) color"
                    );
                }
            }
        }
    }

    #[test]
    fn one_sector_ring_is_mirror_symmetric() {
        assert_mirrored(&ring_rgba(&same_role(1), 36), 36, "n=1");
    }

    #[test]
    fn two_same_role_sectors_are_mirror_symmetric() {
        assert_mirrored(&ring_rgba(&same_role(2), 36), 36, "n=2");
    }
}

mod lit_sector {
    use super::*;

    const COUNTS: [usize; 4] = [4, 5, 8, 16];

    mod middle {
        use super::*;

        #[test]
        fn of_the_lit_sector_is_mostly_white_and_opaque() {
            // The outline eats part of a small sector; the middle stays far above the outline colour (sizes 36 and 22).
            for side in [36, 22] {
                for n in COUNTS {
                    for k in [0, 1, n - 1] {
                        let [r, g, b, a] = middle_pixel(&lit_grey(n, k, side), side, n, k);
                        assert!(
                            [r, g, b].iter().all(|&c| c >= 100),
                            "{side} n={n} k={k}: {r} {g} {b}"
                        );
                        assert!(a >= opaque(side), "{side} n={n} k={k}: alpha {a}");
                    }
                }
            }
        }

        #[test]
        fn of_the_other_sectors_keeps_the_role_colour() {
            for side in SIDES {
                for n in COUNTS {
                    for k in [0, 1, n - 1] {
                        let data = lit_grey(n, k, side);
                        for j in (0..n).filter(|&j| j != k) {
                            let [r, g, b, a] = middle_pixel(&data, side, n, j);
                            assert_eq!(
                                [r, g, b],
                                role_rgb(RingRole::Offline),
                                "{side} n={n} lit={k} j={j}"
                            );
                            assert!(a >= opaque(side), "{side} n={n} lit={k} j={j}: alpha {a}");
                        }
                    }
                }
            }
        }
    }

    mod geometry {
        use super::*;

        #[test]
        fn gaps_next_to_the_lit_sector_stay() {
            let data = lit_grey(8, 2, 36);
            for border in [2, 3] {
                let alpha = pixel_at(&data, 36, TAU * f64::from(border) / 8.0, rm(36))[3];
                assert!(alpha <= 160, "border {border}: alpha {alpha}");
            }
        }

        #[test]
        fn alpha_equals_the_unlit_ring_for_uniform_and_mixed_roles() {
            for side in SIDES {
                for ring in rings() {
                    let plain = alpha_of(&ring_rgba(&ring, side));
                    let last = ring.len() - 1;
                    for k in [0, last.min(1), last] {
                        let lit = alpha_of(&ring_rgba_lit(&ring, Some(k), side));
                        assert_eq!(lit, plain, "{side} {ring:?} lit={k}");
                    }
                }
            }
        }
    }

    mod lit_index {
        use super::*;

        #[test]
        fn none_equals_the_plain_raster() {
            for side in SIDES {
                for ring in rings() {
                    assert_eq!(
                        ring_rgba_lit(&ring, None, side),
                        ring_rgba(&ring, side),
                        "{side} {ring:?}"
                    );
                }
            }
        }

        #[test]
        fn past_the_end_is_the_same_as_none() {
            for side in SIDES {
                for ring in rings() {
                    let plain = ring_rgba(&ring, side);
                    for k in [ring.len(), ring.len() + 1, usize::MAX] {
                        assert_eq!(
                            ring_rgba_lit(&ring, Some(k), side),
                            plain,
                            "{side} {ring:?} lit={k}"
                        );
                    }
                }
            }
        }

        #[test]
        fn on_an_empty_ring_stays_transparent() {
            for side in SIDES {
                let data = ring_rgba_lit(&Ring::EMPTY, Some(0), side);
                assert_eq!(data.len(), (side * side * 4) as usize);
                assert!(data.iter().all(|&byte| byte == 0), "{side}");
            }
        }
    }

    mod outline {
        use super::*;

        fn rgb(px: [u8; 4]) -> [u8; 3] {
            [px[0], px[1], px[2]]
        }

        /// The angle a quarter pixel past the gap at a radial border of the 8-sector ring, at the middle radius.
        fn near_border(side: u32, from_end: bool) -> f64 {
            let step = TAU / 8.0;
            let ring = Ring::uniform(RingRole::Offline, 8);
            let d = (Shape::new(&ring, Some(3), side).half_gap_px + 0.25) / rm(side);
            if from_end {
                step - d
            } else {
                d
            }
        }

        #[test]
        fn middle_of_the_lit_sector_is_white() {
            let data = lit_grey(8, 0, 36);
            assert_eq!(rgb(middle_pixel(&data, 36, 8, 0)), LIT_RGB);
            let small = lit_grey(8, 0, 22);
            assert!(middle_pixel(&small, 22, 8, 0)[0] >= 200);
        }

        /// The slot of the point at `angle` and distance `r` from the centre, for an 8-sector ring with sector 3 lit.
        fn slot_at_polar(side: u32, angle: f64, r: f64) -> Option<usize> {
            let ring = Ring::uniform(RingRole::Offline, 8);
            Shape::new(&ring, Some(3), side).sample(r * angle.sin(), -r * angle.cos())
        }

        #[test]
        fn outer_edge_and_radial_border_take_the_outline_slot() {
            for side in [36, 22] {
                let outer = f64::from(side) * 16.0 / 36.0 - 0.3;
                let start = TAU * 3.0 / 8.0;
                assert_eq!(
                    slot_at_polar(side, start + TAU / 16.0, outer),
                    Some(OUTLINE_SLOT),
                    "{side}"
                );
                for from_end in [false, true] {
                    let angle = start + near_border(side, from_end);
                    assert_eq!(
                        slot_at_polar(side, angle, rm(side)),
                        Some(OUTLINE_SLOT),
                        "{side} {from_end}"
                    );
                }
                assert_eq!(
                    slot_at_polar(side, start + TAU / 16.0, rm(side)),
                    Some(LIT_SLOT),
                    "{side}"
                );
            }
        }

        #[test]
        fn edges_are_darker_than_the_middle_at_22() {
            let data = lit_grey(8, 3, 22);
            let angle = TAU * 3.0 / 8.0 + near_border(22, false);
            let edge = pixel_at(&data, 22, angle, rm(22))[0];
            assert!(edge < middle_pixel(&data, 22, 8, 3)[0]);
            assert!(edge <= 230, "{edge}");
        }

        #[test]
        fn neighbours_are_plain_grey() {
            for side in [36, 22] {
                let data = lit_grey(8, 3, side);
                let grey = rgb(middle_pixel(
                    &ring_rgba(&Ring::uniform(RingRole::Offline, 8), side),
                    side,
                    8,
                    2,
                ));
                for k in [2, 4] {
                    assert_eq!(rgb(middle_pixel(&data, side, 8, k)), grey, "{side} {k}");
                }
            }
        }

        #[test]
        fn alpha_is_the_same_as_without_the_lit_sector() {
            for side in [36, 22] {
                let plain = ring_rgba(&Ring::uniform(RingRole::Offline, 8), side);
                assert_eq!(alpha_of(&lit_grey(8, 0, side)), alpha_of(&plain), "{side}");
            }
        }
    }

    #[test]
    fn white_is_not_a_role_colour() {
        assert_eq!(LIT_RGB, [255, 255, 255]);
        assert!(RING_ROLES.iter().all(|&role| role_rgb(role) != LIT_RGB));
    }

    /// The pixel in the middle of sector `k` of an `n`-sector ring, on the middle radius.
    fn middle_pixel(data: &[u8], side: u32, n: usize, k: usize) -> [u8; 4] {
        pixel_at(data, side, TAU * (k as f64 + 0.5) / n as f64, rm(side))
    }

    /// The raster of an `n`-sector all-grey ring with sector `k` lit.
    fn lit_grey(n: usize, k: usize, side: u32) -> Vec<u8> {
        ring_rgba_lit(&Ring::uniform(RingRole::Offline, n as u16), Some(k), side)
    }

    /// The least alpha of a sector-middle pixel at this size (as in `sector_middles_have_role_colour_and_are_opaque`).
    fn opaque(side: u32) -> u8 {
        if side == 36 {
            255
        } else {
            200
        }
    }

    fn alpha_of(data: &[u8]) -> Vec<u8> {
        data.as_chunks::<4>().0.iter().map(|px| px[3]).collect()
    }

    /// All-grey rings of `COUNTS` sectors and the mixed-role rings of `CASES` (with same-role neighbours).
    fn rings() -> Vec<Ring> {
        let grey = COUNTS.map(|n| Ring::uniform(RingRole::Offline, n as u16));
        grey.into_iter()
            .chain(CASES.map(|(_, roles)| ring_of(roles)))
            .collect()
    }
}
