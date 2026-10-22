//! Tests of the icon picture: carrying the ring over from `View`, the raster, equality.

use super::*;
use crate::gui::tray::app::view::{NetLink, NodeState, PageState};
use crate::gui::tray::app::view_disk::Disk;
use crate::gui::tray::app::view_ring::RingRole;
use crate::gui::tray::app::view_ring_paint::ring_rgba;

const COLORS: [TrayColor; 4] = [
    TrayColor::Grey,
    TrayColor::Red,
    TrayColor::Green,
    TrayColor::GreenOutline,
];

fn ring3() -> Ring {
    Ring::EMPTY
        .add(RingRole::ActiveExit)
        .add(RingRole::ExitClient)
        .add(RingRole::Online)
}

fn view_with(color: TrayColor, ring: Ring) -> View {
    View {
        color,
        page: PageState::Ready,
        node: NodeState::Running,
        ring,
        disk: None,
        net: NetLink::NoNetwork,
    }
}

mod picture {
    use super::*;

    #[test]
    fn empty_ring_gives_no_ring() {
        let picture = icon_picture(view_with(TrayColor::Red, Ring::EMPTY));
        assert_eq!(picture.ring, None);
        assert_eq!(picture.color, TrayColor::Red);
    }

    #[test]
    fn ring_is_carried_for_every_colour() {
        for color in COLORS {
            let picture = icon_picture(view_with(color, ring3()));
            assert_eq!(picture.ring, Some(ring3()));
            assert_eq!(picture.color, color);
        }
    }

    #[test]
    fn same_view_gives_equal_pictures() {
        let a = icon_picture(view_with(TrayColor::Green, ring3()));
        let b = icon_picture(view_with(TrayColor::Green, ring3()));
        assert_eq!(a, b);
    }
}

mod raster {
    use super::*;

    #[test]
    fn solid_picture_has_no_raster() {
        let picture = icon_picture(view_with(TrayColor::Grey, Ring::EMPTY));
        assert_eq!(picture_rgba(&picture, 36), None);
    }

    #[test]
    fn ring_raster_is_unchanged() {
        let picture = icon_picture(view_with(TrayColor::Green, ring3()));
        assert_eq!(picture_rgba(&picture, 36), Some(ring_rgba(&ring3(), 36)));
    }
}

mod with_disk {
    use super::*;

    fn disk6() -> Disk {
        Disk {
            step: 6,
            blue_twelfths: 6,
        }
    }

    fn disk_view(ring: Ring) -> View {
        View {
            disk: Some(disk6()),
            ..view_with(TrayColor::Green, ring)
        }
    }

    #[test]
    fn disk_is_dropped_without_ring() {
        assert_eq!(icon_picture(disk_view(Ring::EMPTY)).disk, None);
    }

    #[test]
    fn disk_paints_only_the_centre() {
        let picture = icon_picture(disk_view(ring3()));
        assert_eq!(picture.disk, Some(disk6()));
        let painted = picture_rgba(&picture, 36).expect("raster");
        let plain = ring_rgba(&ring3(), 36);
        let at = |x: u32, y: u32| ((y * 36 + x) * 4) as usize..((y * 36 + x) * 4 + 4) as usize;
        for (x, y) in (0..36).flat_map(|x| (0..36).map(move |y| (x, y))) {
            let (dx, dy) = (f64::from(x) + 0.5 - 18.0, f64::from(y) + 0.5 - 18.0);
            if dx.hypot(dy) > 10.0 {
                assert_eq!(painted[at(x, y)], plain[at(x, y)], "pixel ({x},{y})");
            }
        }
        assert_eq!(painted[at(18, 18)][3], 255);
    }
}
