//! The "vote waiting" mark in the centre of the tray icon: an amber disc with a dark exclamation sign.
//!
//! What it does: `paint_alert` draws the mark over an RGBA raster (ring or solid circle) with 8×8
//! subsampling and alpha blending, the same way `view_disk::paint_disk` draws the speed disk; the mark
//! has the size of the largest speed disk, so it replaces the disk.
//! What it does not do: it does not decide when the mark is needed (`members_note::alert_wanted`) and does not
//! choose the base raster (`view_picture::picture_rgba`).
//! Links: called by `view_picture::picture_rgba` when the picture has `alert`.

/// Disc color (amber) and mark color (dark brown); both read on a light and on a dark menu bar.
const DISC_RGB: [u8; 3] = [245, 166, 35];
const MARK_RGB: [u8; 3] = [40, 30, 20];

/// Subsampling along each pixel axis (8×8 points).
const SUBSAMPLES: u32 = 8;
/// The raster side for which the sizes below are defined.
const BASE_SIDE_PX: f64 = 36.0;
/// Disc radius, as the largest speed disk.
const DISC_RADIUS_PX: f64 = 9.0;
/// The bar: half width, top and bottom offsets from the centre (y grows down).
const BAR_HALF_WIDTH_PX: f64 = 1.5;
const BAR_TOP_PX: f64 = -6.0;
const BAR_BOTTOM_PX: f64 = 1.5;
/// The dot: radius and offset from the centre.
const DOT_RADIUS_PX: f64 = 1.6;
const DOT_OFFSET_PX: f64 = 5.0;

/// Draws the mark in the centre of `rgba` (`side×side`) over what is there; touches nothing outside the disc.
pub fn paint_alert(rgba: &mut [u8], side: u32) {
    let scale = f64::from(side) / BASE_SIDE_PX;
    let center = f64::from(side) / 2.0;
    for y in 0..side {
        for x in 0..side {
            let hits = sample_hits(x, y, center, scale);
            let at = ((y * side + x) * 4) as usize;
            blend(&mut rgba[at..at + 4], hits);
        }
    }
}

/// The number of subsampling points of a pixel on the dark mark and on the bare amber disc.
fn sample_hits(x: u32, y: u32, center: f64, scale: f64) -> (u32, u32) {
    (0..SUBSAMPLES * SUBSAMPLES).fold((0, 0), |(mark, disc), i| {
        let px = (f64::from(x) + (f64::from(i % SUBSAMPLES) + 0.5) / f64::from(SUBSAMPLES) - center) / scale;
        let py = (f64::from(y) + (f64::from(i / SUBSAMPLES) + 0.5) / f64::from(SUBSAMPLES) - center) / scale;
        if px.hypot(py) > DISC_RADIUS_PX {
            return (mark, disc);
        }
        match on_mark(px, py) {
            true => (mark + 1, disc),
            false => (mark, disc + 1),
        }
    })
}

/// Whether the point (px, py), in base pixels from the centre, lies on the bar or on the dot.
fn on_mark(px: f64, py: f64) -> bool {
    let on_bar = px.abs() <= BAR_HALF_WIDTH_PX && (BAR_TOP_PX..=BAR_BOTTOM_PX).contains(&py);
    on_bar || px.hypot(py - DOT_OFFSET_PX) <= DOT_RADIUS_PX
}

/// Blends the coverage over the pixel `dst` (RGBA, not premultiplied).
fn blend(dst: &mut [u8], (mark, disc): (u32, u32)) {
    let hits = mark + disc;
    if hits == 0 {
        return;
    }
    let cover = f64::from(hits) / f64::from(SUBSAMPLES * SUBSAMPLES);
    let below = f64::from(dst[3]) / 255.0 * (1.0 - cover);
    let alpha = cover + below;
    for c in 0..3 {
        let top = (f64::from(mark) * f64::from(MARK_RGB[c]) + f64::from(disc) * f64::from(DISC_RGB[c])) / f64::from(hits);
        dst[c] = ((top * cover + f64::from(dst[c]) * below) / alpha).round() as u8;
    }
    dst[3] = (alpha * 255.0).round() as u8;
}

#[cfg(test)]
#[path = "view_alert_tests.rs"]
mod tests;
