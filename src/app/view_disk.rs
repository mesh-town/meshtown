//! The speed disk in the centre of the tray icon: its size follows the total exit traffic, its two sectors follow the share.
//!
//! What it does: `disk_of` turns the `through_peer_bps` / `through_us_bps` fields of the `peers[]` rows
//! (bytes/s, `WEB-API.md` §1.3) into a `Disk` — a step 1..=6 and the blue share in twelfths; `paint_disk`
//! draws it over the ring raster: from 12 o'clock clockwise, first the blue sector (our traffic via a
//! peer), then the lilac one (peers' traffic via us) — the same order and colors as the ring.
//! What it does not do: it does not decide whether to draw a disk without a ring (`view_picture`) and does not know the ring's roles.
//! Links: `View.disk` is assembled by `tray_role/views.rs`; the raster is called by `view_picture::picture_rgba`.

use std::f64::consts::TAU;

use super::snapshot::TraySnapshot;
use super::view_ring::{role_rgb, RingRole};

/// Thresholds of the byte/s sum: the step is the number of thresholds the sum has reached.
pub const DISK_STEPS_BPS: [u64; 6] = [10_000, 40_000, 160_000, 640_000, 2_560_000, 10_240_000];
/// Disk radius (px at side 36) by steps 1..=6.
pub const DISK_RADII_PX: [f64; 6] = [2.0, 3.4, 4.8, 6.2, 7.6, 9.0];

/// Subsampling along each pixel axis (8×8 points), as for the ring.
const SUBSAMPLES: u32 = 8;
/// The raster side for which the radii are defined.
const BASE_SIDE_PX: f64 = 36.0;

/// The disk: a size step and the blue sector's share in twelfths (`0..=12`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Disk {
    pub step: u8,
    pub blue_twelfths: u8,
}

/// The disk from a snapshot; `None` — no `peers` or the sum is below the first threshold. Our own row is not counted.
pub fn disk_of(snap: &TraySnapshot) -> Option<Disk> {
    let peers = snap.peers.as_deref()?;
    let (blue, lilac) = peers.iter().filter(|p| !p.is_self).fold((0u64, 0u64), |(b, l), p| {
        (
            b.saturating_add(p.through_peer_bps.unwrap_or(0)),
            l.saturating_add(p.through_us_bps.unwrap_or(0)),
        )
    });
    let step = step_of(blue.saturating_add(lilac));
    (step > 0).then(|| Disk { step, blue_twelfths: twelfths(blue, lilac) })
}

fn step_of(sum: u64) -> u8 {
    DISK_STEPS_BPS.iter().filter(|&&t| sum >= t).count() as u8
}

/// The blue share in twelfths; when both shares are nonzero it is clamped to `1..=11`.
fn twelfths(blue: u64, lilac: u64) -> u8 {
    let total = u128::from(blue) + u128::from(lilac);
    let raw = ((u128::from(blue) * 12 + total / 2) / total) as u8;
    if blue == 0 || lilac == 0 {
        return raw;
    }
    raw.clamp(1, 11)
}

/// Draws the disk in the centre of `rgba` (`side×side`) over what is there; does not touch anything outside the circle.
pub fn paint_disk(rgba: &mut [u8], side: u32, disk: Disk) {
    let radius = DISK_RADII_PX[usize::from(disk.step.clamp(1, 6)) - 1] * f64::from(side) / BASE_SIDE_PX;
    let center = f64::from(side) / 2.0;
    for y in 0..side {
        for x in 0..side {
            let hits = sample_hits(x, y, center, radius, disk.blue_twelfths);
            let at = ((y * side + x) * 4) as usize;
            over(&mut rgba[at..at + 4], hits);
        }
    }
}

/// The number of subsampling points of a pixel in the blue and in the lilac sector.
fn sample_hits(x: u32, y: u32, center: f64, radius: f64, blue_twelfths: u8) -> (u32, u32) {
    let blue_angle = f64::from(blue_twelfths) / 12.0 * TAU;
    (0..SUBSAMPLES * SUBSAMPLES).fold((0, 0), |(b, l), i| {
        let px = f64::from(x) + (f64::from(i % SUBSAMPLES) + 0.5) / f64::from(SUBSAMPLES) - center;
        let py = f64::from(y) + (f64::from(i / SUBSAMPLES) + 0.5) / f64::from(SUBSAMPLES) - center;
        if px.hypot(py) > radius {
            return (b, l);
        }
        if px.atan2(-py).rem_euclid(TAU) < blue_angle {
            return (b + 1, l);
        }
        (b, l + 1)
    })
}

/// Blends the disk coverage over the pixel `dst` (RGBA, not premultiplied).
fn over(dst: &mut [u8], (blue, lilac): (u32, u32)) {
    let hits = blue + lilac;
    if hits == 0 {
        return;
    }
    let (b, l) = (role_rgb(RingRole::ActiveExit), role_rgb(RingRole::ExitClient));
    let cover = f64::from(hits) / f64::from(SUBSAMPLES * SUBSAMPLES);
    let below = f64::from(dst[3]) / 255.0 * (1.0 - cover);
    let alpha = cover + below;
    for c in 0..3 {
        let disk = (f64::from(blue) * f64::from(b[c]) + f64::from(lilac) * f64::from(l[c])) / f64::from(hits);
        dst[c] = ((disk * cover + f64::from(dst[c]) * below) / alpha).round() as u8;
    }
    dst[3] = (alpha * 255.0).round() as u8;
}

#[cfg(test)]
#[path = "view_disk_tests.rs"]
mod tests;
