//! Tests of the own-overload dot: the centre, the colours, what it leaves untouched and when it is not drawn.

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

const DOT: [u8; 4] = [255, 180, 0, 255];

#[test]
fn badge_is_painted_in_the_centre() {
    let with = raster(twelve().with_badge(true));
    let without = raster(twelve());
    assert_eq!(pixel(&with, 18, 18), DOT);
    assert_ne!(pixel(&without, 18, 18), DOT);
    assert_eq!(pixel(&with, 7, 7), pixel(&without, 7, 7));
}

fn disk_view(badge: bool) -> View {
    View {
        disk: Some(Disk {
            step: 5,
            blue_twelfths: 6,
        }),
        ..view(
            twelve().with_badge(badge),
            TrayColor::Green,
            NodeState::Running,
        )
    }
}

#[test]
fn badge_replaces_the_speed_disk() {
    let with = picture_rgba(&icon_picture(disk_view(true)), ICON_SIDE_PX).expect("raster");
    let without = picture_rgba(&icon_picture(disk_view(false)), ICON_SIDE_PX).expect("raster");
    assert!(icon_picture(disk_view(true)).disk.is_some());
    assert_eq!(pixel(&with, 18, 18), DOT);
    assert_ne!(pixel(&without, 18, 18), DOT);
}

#[test]
fn alert_hides_the_badge() {
    let v = view(
        twelve().with_badge(true),
        TrayColor::Green,
        NodeState::Running,
    );
    let picture = icon_picture(v).with_alert(true);
    assert!(picture.alert && picture.badge);
    let rgba = picture_rgba(&picture, ICON_SIDE_PX).expect("raster");
    let alert_only = picture_rgba(
        &icon_picture(view(twelve(), TrayColor::Green, NodeState::Running)).with_alert(true),
        ICON_SIDE_PX,
    )
    .expect("raster");
    assert_eq!(rgba, alert_only);
    assert!(rgba.chunks(4).all(|p| p != DOT));
}

#[test]
fn badge_leaves_the_ring_untouched() {
    let painted = raster(twelve().with_badge(true));
    let plain = raster(twelve());
    // Pixels farther than 10 base px from the centre (17.5, 17.5) are the same with and without the dot.
    for y in 0..36u32 {
        for x in 0..36u32 {
            let d = (f64::from(x) + 0.5 - 18.0).hypot(f64::from(y) + 0.5 - 18.0);
            if d > 10.0 {
                assert_eq!(pixel(&painted, x, y), pixel(&plain, x, y), "{x},{y}");
            }
        }
    }
}

#[test]
fn no_badge_when_grey() {
    let grey = view(Ring::EMPTY, TrayColor::Grey, NodeState::Absent);
    let picture = icon_picture(grey).with_idle(grey, IdleRing::default());
    assert!(!picture.badge);
    let rgba = picture_rgba(&picture, ICON_SIDE_PX).expect("idle ring");
    assert_ne!(pixel(&rgba, 18, 18), DOT);
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
