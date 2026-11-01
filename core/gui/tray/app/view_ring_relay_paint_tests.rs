//! Tests of the dashed sector (the exit reached through another member): gaps, colour, short arcs.

use super::*;

const SIDE: u32 = 36;

fn relay_ring(n: usize) -> Ring {
    (0..n).fold(Ring::EMPTY, |ring, _| ring.add(RingRole::ExitViaRelay))
}

/// The pixel on the middle of the ring at arc distance `s` px from 12 o'clock.
fn at_arc(data: &[u8], s: f64) -> [u8; 4] {
    let r = rm(SIDE);
    pixel_at(data, SIDE, s / r, r)
}

#[test]
fn relay_arc_has_transparent_gaps() {
    let data = ring_rgba(&relay_ring(1), SIDE);
    // 84.8 px of arc: 14 strokes of ~4.2 px with 2 px gaps; the first stroke is [0, 4.2], the first gap [4.2, 6.2].
    let stroke = at_arc(&data, 2.0);
    assert_eq!(stroke[3], 255);
    assert_eq!(stroke[..3], role_rgb(RingRole::ActiveExit));
    // A 2 px gap does not align with the pixel grid: the middle pixel is almost, not exactly, empty.
    assert!(at_arc(&data, 5.2)[3] < 64, "{:?}", at_arc(&data, 5.2));
    assert!(at_arc(&data, 11.4)[3] < 64);
    assert_eq!(at_arc(&data, 8.4)[3], 255);
}

#[test]
fn relay_arc_starts_and_ends_with_a_stroke() {
    let data = ring_rgba(&relay_ring(1), SIDE);
    assert_eq!(at_arc(&data, 0.5)[3], 255);
    assert_eq!(at_arc(&data, 84.3)[3], 255);
}

#[test]
fn short_relay_arc_is_solid() {
    // 8 sectors: the arc is 10.6 px, below the dash threshold.
    let data = ring_rgba(&relay_ring(8), SIDE);
    let solid = ring_rgba(&same_role(8), SIDE);
    let alpha = |d: &[u8]| d.chunks(4).map(|px| px[3]).collect::<Vec<_>>();
    assert_eq!(alpha(&data), alpha(&solid));
}

#[test]
fn plain_exit_is_not_dashed() {
    let data = ring_rgba(&ring_of(&[RingRole::ActiveExit]), SIDE);
    assert_eq!(at_arc(&data, 5.2)[3], 255);
}
