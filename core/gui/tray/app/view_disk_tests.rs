//! Tests of the speed disk: steps, sector shares, the raster.

use super::*;
use serde_json::json;

fn snap(peers: serde_json::Value) -> TraySnapshot {
    TraySnapshot::from_value(&json!({ "peers": peers }))
}

fn row(peer_bps: u64, us_bps: u64) -> serde_json::Value {
    json!({ "through_peer_bps": peer_bps, "through_us_bps": us_bps })
}

fn disk_for(peer_bps: u64, us_bps: u64) -> Option<Disk> {
    disk_of(&snap(json!([row(peer_bps, us_bps)])))
}

mod steps {
    use super::*;

    #[test]
    fn below_ten_kb_has_no_disk() {
        assert_eq!(disk_for(9_999, 0), None);
    }

    #[test]
    fn threshold_starts_step_one() {
        assert_eq!(disk_for(10_000, 0).map(|d| d.step), Some(1));
    }

    #[test]
    fn each_fourfold_adds_a_step() {
        let sums = [40_000, 160_000, 640_000, 2_560_000, 10_240_000];
        for (i, sum) in sums.into_iter().enumerate() {
            assert_eq!(disk_for(sum, 0).map(|d| d.step), Some(i as u8 + 2), "{sum}");
        }
    }

    #[test]
    fn huge_rate_stays_at_top_step() {
        let snap = snap(json!([row(u64::MAX, 0), row(0, u64::MAX)]));
        assert_eq!(disk_of(&snap).map(|d| d.step), Some(6));
    }

    #[test]
    fn own_row_is_ignored() {
        let snap = snap(json!([{ "self": true, "through_peer_bps": 1_000_000 }]));
        assert_eq!(disk_of(&snap), None);
    }

    #[test]
    fn no_peers_array_has_no_disk() {
        assert_eq!(disk_of(&TraySnapshot::from_value(&json!({}))), None);
    }
}

mod shares {
    use super::*;

    #[test]
    fn only_through_peer_is_all_blue() {
        assert_eq!(disk_for(50_000, 0).map(|d| d.blue_twelfths), Some(12));
    }

    #[test]
    fn only_through_us_is_all_lilac() {
        assert_eq!(disk_for(0, 50_000).map(|d| d.blue_twelfths), Some(0));
    }

    #[test]
    fn tiny_share_keeps_minimum_angle() {
        assert_eq!(disk_for(1_000, 99_000).map(|d| d.blue_twelfths), Some(1));
        assert_eq!(disk_for(99_000, 1_000).map(|d| d.blue_twelfths), Some(11));
    }
}

mod paint {
    use super::*;

    const SIDE: u32 = 36;

    fn painted(step: u8, blue_twelfths: u8) -> Vec<u8> {
        let mut rgba = vec![0u8; (SIDE * SIDE * 4) as usize];
        paint_disk(
            &mut rgba,
            SIDE,
            Disk {
                step,
                blue_twelfths,
            },
        );
        rgba
    }

    fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
        let at = ((y * SIDE + x) * 4) as usize;
        [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
    }

    #[test]
    fn step_radius_matches_table() {
        for step in 1..=6u8 {
            let r = DISK_RADII_PX[usize::from(step) - 1];
            let rgba = painted(step, 12);
            let inside = 18 + (r - 1.5).floor() as u32;
            let outside = 18 + (r + 0.5).ceil() as u32;
            assert_eq!(pixel(&rgba, 18, 18)[3], 255, "step {step} centre");
            assert_eq!(
                pixel(&rgba, inside, 18)[3],
                255,
                "step {step} inside r = {r}"
            );
            assert_eq!(
                pixel(&rgba, outside, 18)[3],
                0,
                "step {step} outside r = {r}"
            );
        }
    }

    /// The colour at `radius` px from the centre, `degrees` clockwise from twelve o'clock.
    fn at_angle(rgba: &[u8], radius: f64, degrees: f64) -> [u8; 3] {
        let a = degrees.to_radians();
        let (x, y) = (18.0 + radius * a.sin(), 18.0 - radius * a.cos());
        let p = pixel(rgba, x.floor() as u32, y.floor() as u32);
        [p[0], p[1], p[2]]
    }

    /// Whether two colours are the same up to the rounding of the edge blending.
    fn near(a: [u8; 3], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 3)
    }

    #[test]
    fn tiny_share_sector_is_thirty_degrees() {
        let rgba = painted(6, 1);
        let blue = role_rgb(RingRole::ActiveExit);
        let lilac = role_rgb(RingRole::ExitClient);
        assert!(near(at_angle(&rgba, 6.0, 15.0), blue), "15.0 degrees");
        assert!(near(at_angle(&rgba, 6.0, 22.0), blue), "22.0 degrees");
        assert!(near(at_angle(&rgba, 6.0, 38.0), lilac), "38.0 degrees");
        assert!(near(at_angle(&rgba, 6.0, 90.0), lilac), "90.0 degrees");
        assert!(near(at_angle(&rgba, 6.0, -15.0), lilac), "-15.0 degrees");
        let count = |c: [u8; 3]| {
            rgba.chunks(4)
                .filter(|p| p[3] == 255 && near([p[0], p[1], p[2]], c))
                .count() as f64
        };
        let share = count(blue) / (count(blue) + count(lilac));
        assert!((share - 1.0 / 12.0).abs() < 0.015, "blue share {share}");
    }

    #[test]
    fn blue_sector_starts_at_twelve() {
        let rgba = painted(6, 6);
        let blue = role_rgb(RingRole::ActiveExit);
        let lilac = role_rgb(RingRole::ExitClient);
        assert_eq!(pixel(&rgba, 22, 14)[..3], blue);
        assert_eq!(pixel(&rgba, 13, 14)[..3], lilac);
    }

    #[test]
    fn single_colour_disk_is_mirror_symmetric() {
        let rgba = painted(6, 12);
        for y in 0..SIDE {
            for x in 0..SIDE {
                assert_eq!(
                    pixel(&rgba, x, y),
                    pixel(&rgba, x, SIDE - 1 - y),
                    "({x},{y})"
                );
            }
        }
    }

    #[test]
    fn disk_does_not_touch_far_pixels() {
        let rgba = painted(6, 6);
        for y in 0..SIDE {
            for x in 0..SIDE {
                let (dx, dy) = (f64::from(x) + 0.5 - 18.0, f64::from(y) + 0.5 - 18.0);
                if dx.hypot(dy) >= 10.0 {
                    assert_eq!(pixel(&rgba, x, y), [0; 4], "({x},{y})");
                }
            }
        }
    }
}
