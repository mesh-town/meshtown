//! The tray icon picture: color, ring of sectors and speed disk — the key and the input of the raster.
//!
//! What it does: `IconPicture` is everything the icon's look in the tray depends on; `icon_picture` builds it
//! from a `View`, `picture_rgba` draws the ring raster with the disk on top (a solid icon has no raster).
//! `IconPicture::with_idle` adds the grey ring of a node that is not running (stopped, starting, stopping),
//! and for a starting node the highlighted sector of the spinner; `idle_sectors` picks the sector count.
//! What it does not do: it does not put the icon in the tray (`tray/menu_build.rs`) and keeps no icon cache.
//! Links: equality of `IconPicture` is the "icon already in place" key of `TrayUi`; the ring is `view_ring`,
//! the ring raster is `view_ring_paint`.

use super::view::{icon_rgba, NodeState, PageState, TrayColor, View, ICON_SIDE_PX};
use super::view_alert::paint_alert;
use super::view_disk::{paint_disk, Disk};
use super::view_ring::{ring_to_draw, Ring, RingRole};
use super::view_ring_paint::ring_rgba_lit;

/// The icon picture: color, ring of sectors (`None` means a solid icon), the speed disk in the centre
/// (only with a ring) and the "vote waiting" mark that replaces the disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconPicture {
    pub color: TrayColor,
    pub ring: Option<Ring>,
    pub disk: Option<Disk>,
    pub alert: bool,
    /// The number of the white (highlighted) sector of the ring, if any.
    pub lit: Option<u16>,
}

/// The sector count of the idle ring when the last running count is not known.
pub const IDLE_SECTORS: u16 = 8;
/// The fewest sectors of a spinning ring: with fewer the white one covers too much of it.
pub const SPIN_MIN_SECTORS: u16 = 4;
/// The most sectors of a spinning ring: beyond it the gaps between sectors disappear.
pub const SPIN_MAX_SECTORS: u16 = 16;

/// What the tray remembers for the ring of a node that is not running.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdleRing {
    /// The sector count of the last running ring; 0 means unknown.
    pub remembered: u16,
    /// The number of the spinner frame; grows by one per tick.
    pub frame: u32,
}

/// The sector count of the idle ring: the remembered one or `IDLE_SECTORS`; clamped for `Starting`.
pub fn idle_sectors(node: NodeState, remembered: u16) -> u16 {
    let n = if remembered == 0 { IDLE_SECTORS } else { remembered };
    match node {
        NodeState::Starting => n.clamp(SPIN_MIN_SECTORS, SPIN_MAX_SECTORS),
        _ => n,
    }
}

/// The spinner turns while the node is `Starting` and its page is not reported unavailable (a stuck start).
pub fn spinning(view: View) -> bool {
    view.node == NodeState::Starting && view.page != PageState::Unavailable
}

impl IconPicture {
    /// The grey ring for a node that is `Absent`, `Starting` or `Stopping` and has no ring of its own; a starting
    /// node also gets the white sector `frame % n`. A start with an unavailable page stays a solid icon. Any other
    /// picture is unchanged.
    pub fn with_idle(self, view: View, idle: IdleRing) -> Self {
        let node = view.node;
        let idle_node = matches!(node, NodeState::Absent | NodeState::Stopping) || spinning(view);
        if self.ring.is_some() || !idle_node {
            return self;
        }
        let n = idle_sectors(node, idle.remembered);
        let lit = (node == NodeState::Starting).then(|| (idle.frame % u32::from(n)) as u16);
        Self { ring: Some(Ring::uniform(RingRole::Offline, n)), lit, ..self }
    }

    /// The picture with the "vote waiting" mark when `alert` is true (the disk is dropped); otherwise unchanged.
    pub fn with_alert(self, alert: bool) -> Self {
        match alert {
            true => Self { alert, disk: None, ..self },
            false => self,
        }
    }
}

/// The icon picture for a view; the ring is as in `ring_to_draw`, the disk is `view.disk`, if there is a ring.
pub fn icon_picture(view: View) -> IconPicture {
    let ring = ring_to_draw(view);
    let disk = view.disk.filter(|_| ring.is_some());
    IconPicture { color: view.color, ring, disk, alert: false, lit: None }
}

/// The `side`×`side` RGBA raster; `None` without a ring and without the mark — the icon is solid and comes
/// from the cache by color. With the mark there is always a raster (a solid base only for `ICON_SIDE_PX`).
pub fn picture_rgba(picture: &IconPicture, side: u32) -> Option<Vec<u8>> {
    let mut rgba = base_rgba(picture, side)?;
    if picture.alert {
        paint_alert(&mut rgba, side);
        return Some(rgba);
    }
    if let Some(disk) = picture.disk {
        paint_disk(&mut rgba, side, disk);
    }
    Some(rgba)
}

/// The raster under the mark or disk: the ring, or (with the mark only) the solid circle of the color.
fn base_rgba(picture: &IconPicture, side: u32) -> Option<Vec<u8>> {
    if let Some(ring) = picture.ring.as_ref() {
        return Some(ring_rgba_lit(ring, picture.lit.map(usize::from), side));
    }
    (picture.alert && side == ICON_SIDE_PX).then(|| icon_rgba(picture.color))
}

#[cfg(test)]
#[path = "view_picture_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "view_picture_idle_tests.rs"]
mod idle_tests;
