//! The raster of the ring of sectors by member (`Ring`): geometry, gaps, smoothing by subsampling.
//!
//! What it does: draws a `Ring` into a `side×side` RGBA square. The centre is `side/2`, the outer radius
//! `side·16/36`, the thickness is whole pixels per `ring_spec`; pixel coverage is 8×8 subsampling, the pixel's RGB
//! is the average of the role colors weighted by coverage (a border between different roles is blended, not
//! a step). Sectors run clockwise from 12 o'clock, identical neighbours form one arc with a narrow gap.
//! What it does not do: it does not decide whether to show sectors (`view_ring::ring_to_draw`) and does not place the icon.
//! Links: `tray/menu_build.rs::view_icon` reaches `ring_rgba_lit` through `view_picture::picture_rgba` at
//! `ICON_SIDE_PX`; in production there is one size (36), the sizes 22 and 16 appear only in tests.

use std::f64::consts::{FRAC_PI_2, TAU};

use super::view_ring::{role_rgb, Ring, RING_ROLES, RING_ROLE_COUNT};

/// Subsampling along each pixel axis (8×8 points).
const SUBSAMPLES: u32 = 8;

/// The smallest sector arc length along the middle of the ring (px) at which a gap is drawn between identical
/// neighbours. Dozens of members give short arcs: a gap at every border
/// would turn the ring into a dotted line, so below the threshold the ring stays solid. The threshold is 5 px:
/// after a gap 1 px wide, a stroke of at least ~4 px remains of the arc. At side 36 the arc along the
/// middle of the ring is 84.8/n px, so there are gaps up to and including 16 members; from 17 the ring is
/// solid.
const MIN_GAP_ARC_PX: f64 = 5.0;

/// From this icon size the ring is thick and has a gap (`ring_spec`); below it, thin and solid.
const FULL_SIDE_PX: u32 = 36;

/// The ring thickness and the gap width between identical neighbours, whole pixels.
pub struct RingSpec {
    pub thickness_px: u32,
    pub gap_px: u32,
}

/// The only size table: `side_px ≥ 36` — 5/1, smaller — 3/0.
pub fn ring_spec(side_px: u32) -> RingSpec {
    if side_px >= FULL_SIDE_PX {
        return RingSpec {
            thickness_px: 5,
            gap_px: 1,
        };
    }
    RingSpec {
        thickness_px: 3,
        gap_px: 0,
    }
}

/// The colour of the highlighted sector: pure white.
pub const LIT_RGB: [u8; 3] = [255, 255, 255];

/// The colour slot of the highlighted sector; the slots before it are the roles.
const LIT_SLOT: usize = RING_ROLE_COUNT;

/// The colour of the outline of the highlighted sector: dark grey.
pub const LIT_OUTLINE_RGB: [u8; 3] = [40, 40, 40];

/// The colour slot of the outline of the highlighted sector.
const OUTLINE_SLOT: usize = RING_ROLE_COUNT + 1;

/// The number of colour slots: every role, the highlight and its outline.
const SLOT_COUNT: usize = RING_ROLE_COUNT + 2;

/// The outline width at the full icon size (px); it scales with the icon.
const OUTLINE_PX_AT_FULL: f64 = 0.75;

/// A `side×side` RGBA raster; an empty ring is all zero bytes.
#[cfg(test)]
pub fn ring_rgba(ring: &Ring, side: u32) -> Vec<u8> {
    ring_rgba_lit(ring, None, side)
}

/// Like `ring_rgba`, with sector `lit` filled `LIT_RGB`; `lit` past the end is the same as `None`. The gaps
/// follow the roles only, so the geometry does not depend on the highlight.
pub fn ring_rgba_lit(ring: &Ring, lit: Option<usize>, side: u32) -> Vec<u8> {
    if ring.is_empty() {
        return vec![0; (side * side * 4) as usize];
    }
    let shape = Shape::new(ring, lit.filter(|&k| k < ring.len()), side);
    let mut out = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            out.extend_from_slice(&shape.pixel(x, y));
        }
    }
    out
}

/// A ring of one size, ready for sampling points. Angles run clockwise from 12 o'clock.
struct Shape<'a> {
    ring: &'a Ring,
    /// The highlighted sector, if any.
    lit: Option<usize>,
    sector_count: usize,
    center_px: f64,
    outer_radius_px: f64,
    inner_radius_px: f64,
    sector_angle_rad: f64,
    /// 0 means no gaps.
    half_gap_px: f64,
    /// The width of the outline of the highlighted sector, at least half a pixel.
    outline_px: f64,
}

impl<'a> Shape<'a> {
    fn new(ring: &'a Ring, lit: Option<usize>, side_px: u32) -> Self {
        let spec = ring_spec(side_px);
        let sector_count = ring.len();
        let outer_radius_px = f64::from(side_px) * 16.0 / 36.0;
        let mid = outer_radius_px - f64::from(spec.thickness_px) / 2.0;
        let sector_angle_rad = TAU / sector_count as f64;
        Shape {
            ring,
            lit,
            sector_count,
            center_px: f64::from(side_px) / 2.0,
            outer_radius_px,
            inner_radius_px: outer_radius_px - f64::from(spec.thickness_px),
            sector_angle_rad,
            half_gap_px: half_gap_px(spec.gap_px, sector_count, sector_angle_rad * mid),
            outline_px: (f64::from(side_px) / f64::from(FULL_SIDE_PX) * OUTLINE_PX_AT_FULL).max(0.5),
        }
    }

    /// The RGBA of pixel `(x, y)`.
    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let hits = (0..SUBSAMPLES * SUBSAMPLES)
            .filter_map(|i| {
                self.sample(self.point(x, i % SUBSAMPLES), self.point(y, i / SUBSAMPLES))
            })
            .fold([0u32; SLOT_COUNT], |mut hits, slot| {
                hits[slot] += 1;
                hits
            });
        blend(hits)
    }

    /// The coordinate of subsample point `sub` inside pixel `cell`, from the raster centre.
    fn point(&self, cell: u32, sub: u32) -> f64 {
        f64::from(cell) + (f64::from(sub) + 0.5) / f64::from(SUBSAMPLES) - self.center_px
    }

    /// The colour slot at point `(px, py)` from the centre (`py` points down); `None` — outside the ring or in a gap.
    fn sample(&self, px: f64, py: f64) -> Option<usize> {
        let r = px.hypot(py);
        let theta = px.atan2(-py).rem_euclid(TAU);
        let k = ((theta / self.sector_angle_rad).floor() as usize).min(self.sector_count - 1);
        let in_ring = (self.inner_radius_px..=self.outer_radius_px).contains(&r);
        (in_ring && !self.in_gap(k, theta, r)).then(|| self.slot_with_outline(k, theta, r))
    }

    /// The slot of sector `k`; a point of the highlighted sector near any of its edges takes the outline slot.
    fn slot_with_outline(&self, k: usize, theta: f64, r: f64) -> usize {
        let slot = self.slot_at(k);
        if slot == LIT_SLOT && self.near_edge(k, theta, r) {
            return OUTLINE_SLOT;
        }
        slot
    }

    /// A point of sector `k` is closer than the outline width to the inner arc, the outer arc or a radial border.
    fn near_edge(&self, k: usize, theta: f64, r: f64) -> bool {
        let arcs = (r - self.inner_radius_px).min(self.outer_radius_px - r);
        arcs < self.outline_px || (self.sector_count > 1 && self.near_radial_border(k, theta, r))
    }

    /// The distance from the point to the nearer radial border of sector `k` is less than the outline width past
    /// the half gap (the outline starts where the gap ends).
    fn near_radial_border(&self, k: usize, theta: f64, r: f64) -> bool {
        let (_, nearest) = self.nearest_edge(k, theta);
        nearest < FRAC_PI_2 && r * nearest.sin() < self.half_gap_px + self.outline_px
    }

    /// The colour slot of sector `k`: the highlight, or the slot of its role.
    fn slot_at(&self, k: usize) -> usize {
        if self.lit == Some(k) {
            return LIT_SLOT;
        }
        self.ring.role_at(k) as usize
    }

    /// A point of sector `k` lies in a gap: at the nearer border the neighbours have the same role, and the distance from
    /// the point to the border radius is less than half the gap (a constant width along the radius).
    fn in_gap(&self, k: usize, theta: f64, r: f64) -> bool {
        let (edge, nearest) = self.nearest_edge(k, theta);
        let near_line = nearest < FRAC_PI_2 && r * nearest.sin() < self.half_gap_px;
        near_line && self.joins_same_role(edge)
    }

    /// The nearer border of sector `k` (the number of the sector it starts; the
    /// last→first seam is 0) and the angle to it.
    fn nearest_edge(&self, k: usize, theta: f64) -> (usize, f64) {
        let to_start = theta - k as f64 * self.sector_angle_rad;
        let to_end = (k + 1) as f64 * self.sector_angle_rad - theta;
        if to_start <= to_end {
            return (k, to_start);
        }
        ((k + 1) % self.sector_count, to_end)
    }

    /// At border `edge` the role of the sector before it is the same as after it.
    fn joins_same_role(&self, edge: usize) -> bool {
        let before = (edge + self.sector_count - 1) % self.sector_count;
        self.ring.role_at(before) == self.ring.role_at(edge)
    }
}

/// Half the gap width: a gap is needed when `gap > 0`, with two or more sectors and a sector arc along the
/// middle of the ring of at least `MIN_GAP_ARC_PX`.
fn half_gap_px(gap: u32, n: usize, mid_arc: f64) -> f64 {
    if gap == 0 || n < 2 || mid_arc < MIN_GAP_ARC_PX {
        return 0.0;
    }
    f64::from(gap) / 2.0
}

/// The colour of every slot: the roles in order, then `LIT_RGB` and `LIT_OUTLINE_RGB`.
fn slot_colors() -> [[u8; 3]; SLOT_COUNT] {
    let mut colors = [LIT_RGB; SLOT_COUNT];
    for (color, role) in colors.iter_mut().zip(RING_ROLES) {
        *color = role_rgb(role);
    }
    colors[OUTLINE_SLOT] = LIT_OUTLINE_RGB;
    colors
}

/// A pixel from the number of subsample hits by role: alpha is the coverage share, RGB is the average of the
/// slot colors weighted by hits; an empty pixel is `[0; 4]`.
fn blend(hits: [u32; SLOT_COUNT]) -> [u8; 4] {
    let total: u32 = hits.iter().sum();
    if total == 0 {
        return [0; 4];
    }
    let sums = slot_colors().into_iter().zip(hits).fold([0u32; 3], |acc, (rgb, n)| {
        [0, 1, 2].map(|c| acc[c] + n * u32::from(rgb[c]))
    });
    let [r, g, b] = sums.map(|sum| ((sum + total / 2) / total) as u8);
    let share = f64::from(total) / f64::from(SUBSAMPLES * SUBSAMPLES);
    [r, g, b, (share.min(1.0) * 255.0).round() as u8]
}

#[cfg(test)]
#[path = "view_ring_paint_tests.rs"]
mod tests;
