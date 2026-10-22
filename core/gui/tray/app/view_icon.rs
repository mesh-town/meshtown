//! The tray icon drawing: a 36×36 RGBA circle with a smoothed edge and the `GreenOutline` ring.
//! What is not here: the colors and menu texts (`view.rs`) and the ring of members (`view_ring_paint.rs`).

use super::{TrayColor, ICON_SIDE_PX};

const RING_WIDTH_PX: f64 = 5.0;

/// The centre of the icon circle in coordinates `x + 0.5`: the middle of the raster `ICON_SIDE_PX / 2` (symmetry
/// of the circle; it used to be 17.5, which shifted the icon half a pixel up and to the left).
const CENTER_PX: f64 = 18.0;
const OUTER_RADIUS_PX: f64 = 16.0;

/// Pixel coverage at distance `dist` from the centre: the circle reaches the outer edge; the ring
/// (`GreenOutline`) is also hollow in the middle, with thickness `RING_WIDTH_PX`.
fn pixel_coverage(color: TrayColor, dist: f64, radius: f64) -> f64 {
    let outer = (radius + 0.5 - dist).clamp(0.0, 1.0);
    if color != TrayColor::GreenOutline {
        return outer;
    }
    outer.min((dist - (radius + 0.5 - RING_WIDTH_PX)).clamp(0.0, 1.0))
}

/// A 36×36 RGBA circle with a smoothed edge: centre (18; 18), radius 16, pixel coverage is
/// `(16.5 − dist).clamp(0, 1)`. RGB is the solid color; alpha is `(coverage × 255).round()`
/// (transparency outside the circle, not blending with the background in advance).
pub fn icon_rgba(color: TrayColor) -> Vec<u8> {
    let (r, g, b) = match color {
        TrayColor::Grey => (140u8, 140u8, 140u8),
        TrayColor::Green | TrayColor::GreenOutline => (60u8, 200u8, 90u8),
        TrayColor::Red => (225u8, 60u8, 50u8),
    };

    let mut out = Vec::with_capacity((ICON_SIDE_PX * ICON_SIDE_PX * 4) as usize);
    for y in 0..ICON_SIDE_PX {
        for x in 0..ICON_SIDE_PX {
            let dx = x as f64 + 0.5 - CENTER_PX;
            let dy = y as f64 + 0.5 - CENTER_PX;
            let dist = (dx * dx + dy * dy).sqrt();
            let coverage = pixel_coverage(color, dist, OUTER_RADIUS_PX);
            let alpha = (coverage * 255.0).round() as u8;
            out.push(r);
            out.push(g);
            out.push(b);
            out.push(alpha);
        }
    }
    out
}
