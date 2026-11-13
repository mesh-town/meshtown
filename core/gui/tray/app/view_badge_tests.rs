//! Tests of the own-overload dot: the corner, the colours, what it leaves untouched and when it is not drawn.

use crate::gui::tray::app::view::{
    icon_rgba, NetLink, NodeState, PageState, TrayColor, View, ICON_SIDE_PX,
};
use crate::gui::tray::app::view_disk::Disk;
use crate::gui::tray::app::view_picture::{icon_picture, picture_rgba, IconPicture, IdleRing};
use crate::gui::tray::app::view_ring::{Ring, RingRole};

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * ICON_SIDE_PX + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

fn twelve() -> Ring {
    (0..11).fold(Ring::uniform(RingRole::ActiveExitBusy, 1), |ring, _| {
        ring.add(RingRole::Online)
    })
}

fn view(ring: Ring, color: TrayColor, node: NodeState) -> View {
    View {
        color,
        page: PageState::Ready,
        node,
        ring,
        disk: None,
        net: NetLink::Joined,
    }
}

fn raster(ring: Ring) -> Vec<u8> {
    let v = view(ring, TrayColor::Green, NodeState::Running);
    picture_rgba(&icon_picture(v), ICON_SIDE_PX).expect("raster")
}

#[test]
fn badge_is_painted_top_left_over_the_ring() {
    let with = raster(twelve().with_badge(true));
    let without = raster(twelve());
    assert_eq!(pixel(&with, 7, 7), [255, 204, 0, 255]);
    assert_ne!(pixel(&without, 7, 7), [255, 204, 0, 255]);
    // The rim (a mix with the dark ring colour) is darker than the body; the far corner of the icon is untouched.
    assert!(pixel(&with, 3, 7)[0] < 160, "{:?}", pixel(&with, 3, 7));
    assert_eq!(pixel(&with, 34, 34), pixel(&without, 34, 34));
}

#[test]
fn badge_leaves_the_first_sector_untouched() {
    let painted = raster(twelve().with_badge(true));
    let plain = raster(twelve());
    // The first sector (12 to 1 o'clock) in the top right quarter is the same with and without the dot.
    for y in 0..18 {
        for x in 18..36 {
            assert_eq!(pixel(&painted, x, y), pixel(&plain, x, y), "{x},{y}");
        }
    }
}

#[test]
fn badge_and_alert_coexist() {
    let v = view(
        twelve().with_badge(true),
        TrayColor::Green,
        NodeState::Running,
    );
    let picture = icon_picture(v).with_alert(true);
    assert!(picture.alert && picture.badge);
    let rgba = picture_rgba(&picture, ICON_SIDE_PX).expect("raster");
    assert_eq!(pixel(&rgba, 7, 7), [255, 204, 0, 255]);
    assert_eq!(pixel(&rgba, 18, 18)[3], 255);
    assert_ne!(pixel(&rgba, 18, 18), pixel(&raster(twelve()), 18, 18));
}

#[test]
fn no_badge_when_grey() {
    let grey = view(Ring::EMPTY, TrayColor::Grey, NodeState::Absent);
    let picture = icon_picture(grey).with_idle(grey, IdleRing::default());
    assert!(!picture.badge);
    let rgba = picture_rgba(&picture, ICON_SIDE_PX).expect("idle ring");
    assert_ne!(pixel(&rgba, 7, 7), [255, 204, 0, 255]);
}

#[test]
fn no_badge_without_a_ring() {
    let v = view(
        Ring::EMPTY.with_badge(true),
        TrayColor::Green,
        NodeState::Running,
    );
    let picture = icon_picture(v);
    assert!(!picture.badge && picture.ring.is_none());
    assert_eq!(picture_rgba(&picture, ICON_SIDE_PX), None);
    let forced = IconPicture {
        badge: true,
        disk: None::<Disk>,
        ..picture
    };
    let solid = icon_rgba(TrayColor::Green);
    let rgba = picture_rgba(&forced, ICON_SIDE_PX);
    assert!(rgba.is_none() || rgba == Some(solid));
}
