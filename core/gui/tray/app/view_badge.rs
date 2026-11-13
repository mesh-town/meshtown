//! The "this computer is an overloaded exit" dot in the centre of the tray icon: an orange disc with a dark rim.
//!
//! What it does: `paint_badge` draws the dot over an RGBA raster that already has the ring with 8×8 subsampling
//! and alpha blending, the same way `view_alert::paint_alert` draws its disc; it takes the centre, where the speed
//! disk would be.
//! What it does not do: it does not decide when the dot is needed (`Ring::badge`, set from `exit.load.overloaded` of
//! all visible networks) and does not draw without a ring (`view_picture::icon_picture` gates that).
//! Links: called by `view_picture::picture_rgba` when the picture has `badge`.

/// Dot colour (orange) and rim colour (dark brown): the rim keeps the dot readable on a light menu bar.
const DOT_RGB: [u8; 3] = [255, 180, 0];
const RIM_RGB: [u8; 3] = [40, 30, 20];

/// Subsampling along each pixel axis (8×8 points).
const SUBSAMPLES: u32 = 8;
/// The raster side for which the sizes below are defined.
const BASE_SIDE_PX: f64 = 36.0;
/// The outer radius in base pixels (as the largest speed disk); the rim is the outer 1 px.
const RADIUS_PX: f64 = 9.0;
const RIM_PX: f64 = 1.0;

/// Draws the dot in the centre of `rgba` (`side×side`) over what is there; touches nothing outside it.
pub fn paint_badge(rgba: &mut [u8], side: u32) {
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

/// The number of subsampling points of a pixel on the rim and on the orange body.
fn sample_hits(x: u32, y: u32, center: f64, scale: f64) -> (u32, u32) {
    (0..SUBSAMPLES * SUBSAMPLES).fold((0, 0), |(rim, body), i| {
        let at =
            |n: u32, v: u32| (f64::from(v) + (f64::from(n) + 0.5) / f64::from(SUBSAMPLES)) / scale;
        let mid = center / scale;
        let dist = (at(i % SUBSAMPLES, x) - mid).hypot(at(i / SUBSAMPLES, y) - mid);
        if dist > RADIUS_PX {
            return (rim, body);
        }
        match dist > RADIUS_PX - RIM_PX {
            true => (rim + 1, body),
            false => (rim, body + 1),
        }
    })
}

/// Blends the coverage over the pixel `dst` (RGBA, not premultiplied).
fn blend(dst: &mut [u8], (rim, body): (u32, u32)) {
    let hits = rim + body;
    if hits == 0 {
        return;
    }
    let cover = f64::from(hits) / f64::from(SUBSAMPLES * SUBSAMPLES);
    let below = f64::from(dst[3]) / 255.0 * (1.0 - cover);
    let alpha = cover + below;
    for c in 0..3 {
        let top = (f64::from(rim) * f64::from(RIM_RGB[c])
            + f64::from(body) * f64::from(DOT_RGB[c]))
            / f64::from(hits);
        dst[c] = ((top * cover + f64::from(dst[c]) * below) / alpha).round() as u8;
    }
    dst[3] = (alpha * 255.0).round() as u8;
}

#[cfg(test)]
#[path = "view_badge_tests.rs"]
mod tests;
