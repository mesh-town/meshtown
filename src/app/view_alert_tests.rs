//! Tests of the "vote waiting" mark: the centre, the untouched rim, no disk colors.

use super::*;
use crate::app::view::{icon_rgba, TrayColor, ICON_SIDE_PX};
use crate::app::view_picture::{picture_rgba, IconPicture};
use crate::app::view_disk::Disk;
use crate::app::view_ring::{role_rgb, Ring, RingRole};

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * ICON_SIDE_PX + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]]
}

fn ring() -> Ring {
    Ring::EMPTY.add(RingRole::ActiveExit).add(RingRole::ExitClient)
}

fn picture(color: TrayColor, ring: Option<Ring>) -> IconPicture {
    let disk = Some(Disk { step: 6, blue_twelfths: 6 });
    IconPicture { color, ring, disk, alert: false, lit: None }.with_alert(true)
}

#[test]
fn with_alert_drops_the_disk_and_sets_the_flag() {
    let p = picture(TrayColor::Green, Some(ring()));
    assert!(p.alert);
    assert_eq!(p.disk, None);
    assert!(p.ring.is_some());
}

#[test]
fn alert_differs_from_no_alert() {
    let base = IconPicture { color: TrayColor::Green, ring: None, disk: None, alert: false, lit: None };
    assert_ne!(base.with_alert(true), base.with_alert(false));
    assert_eq!(base.with_alert(false), base);
}

#[test]
fn centre_is_the_mark_and_has_no_disk_colours_over_a_ring() {
    let rgba = picture_rgba(&picture(TrayColor::Green, Some(ring())), ICON_SIDE_PX).expect("raster");
    // The ring may reach the corners of a bigger square; the disc itself is inside 12..24.
    // The bar is dark, the dot is dark, the disc beside the bar is amber.
    assert_eq!(pixel(&rgba, 18, 14), [40, 30, 20, 255]);
    assert_eq!(pixel(&rgba, 18, 23), [40, 30, 20, 255]);
    assert_eq!(pixel(&rgba, 22, 14), [245, 166, 35, 255]);
    for (role, name) in [(RingRole::ActiveExit, "blue"), (RingRole::ExitClient, "lilac")] {
        let rgb = role_rgb(role);
        for y in 12..24 {
            for x in 12..24 {
                let p = pixel(&rgba, x, y);
                assert!(p[..3] != rgb, "{name} pixel at {x},{y}");
            }
        }
    }
}

#[test]
fn solid_icons_get_the_mark_and_keep_the_rim() {
    for color in [TrayColor::Green, TrayColor::GreenOutline] {
        let rgba = picture_rgba(&picture(color, None), ICON_SIDE_PX).expect("raster");
        let plain = icon_rgba(color);
        assert_eq!(pixel(&rgba, 18, 14), [40, 30, 20, 255]);
        assert_eq!(pixel(&rgba, 22, 14), [245, 166, 35, 255]);
        assert_eq!(pixel(&rgba, 0, 0), pixel(&plain, 0, 0));
        assert_eq!(pixel(&rgba, 2, 18), pixel(&plain, 2, 18));
        assert_eq!(pixel(&rgba, 18, 3), pixel(&plain, 18, 3));
    }
}

#[test]
fn mark_stays_inside_the_disc() {
    let mut rgba = vec![0u8; (ICON_SIDE_PX * ICON_SIDE_PX * 4) as usize];
    paint_alert(&mut rgba, ICON_SIDE_PX);
    assert_eq!(pixel(&rgba, 5, 18)[3], 0);
    assert_eq!(pixel(&rgba, 18, 18 + 10)[3], 0);
}
