//! Tests of the lazy cache of the spinner frames (`frame_cache.rs`). The cache is generic, so the tests use
//! `T = u32`: the built "icon" is the number of the `build` call that made it, which tells a cached frame
//! from a rebuilt one.

use super::*;
use crate::gui::tray::app::view::TrayColor;
use crate::gui::tray::app::view_disk::Disk;
use crate::gui::tray::app::view_picture::IconPicture;
use crate::gui::tray::app::view_ring::{Ring, RingRole};
use std::cell::Cell;

/// A frame with no white sector is a plain ring or a solid icon — it is built in place every time.
mod unlit_key {
    use super::*;

    #[test]
    fn is_not_cached() {
        let mut f = setup();
        assert_eq!(f.get(key(8, None)), Ok(1));
        assert_eq!(f.get(key(8, None)), Ok(2), "the second call builds again");
        assert_eq!(f.builds(), 2);
    }

    #[test]
    fn is_not_cached_without_a_ring_either() {
        let solid = IconPicture {
            ring: None,
            ..key(8, None)
        };
        let mut f = setup();
        f.get(solid).unwrap();
        f.get(solid).unwrap();
        assert_eq!(f.builds(), 2);
    }
}

mod lit_key {
    use super::*;

    #[test]
    fn same_key_is_built_once() {
        let mut f = setup();
        assert_eq!(f.get(key(8, Some(3))), Ok(1));
        assert_eq!(
            f.get(key(8, Some(3))),
            Ok(1),
            "the second call gets the first frame"
        );
        assert_eq!(f.builds(), 1);
    }

    #[test]
    fn other_sector_is_built_separately() {
        let mut f = setup();
        assert_eq!(f.get(key(8, Some(0))), Ok(1));
        assert_eq!(f.get(key(8, Some(1))), Ok(2));
        assert_eq!(
            f.get(key(8, Some(0))),
            Ok(1),
            "the first frame is still there"
        );
        assert_eq!(f.get(key(8, Some(1))), Ok(2));
        assert_eq!(f.builds(), 2);
    }

    #[test]
    fn full_circle_builds_each_frame_once() {
        let mut f = setup();
        for _lap in 0..3 {
            (0..8).for_each(|k| {
                f.get(key(8, Some(k))).unwrap();
            });
        }
        assert_eq!(f.builds(), 8, "three laps of eight sectors");
    }

    #[test]
    fn alert_mark_is_part_of_the_key() {
        let marked = IconPicture {
            alert: true,
            disk: None,
            ..key(8, Some(2))
        };
        let mut f = setup();
        assert_eq!(f.get(key(8, Some(2))), Ok(1));
        assert_eq!(
            f.get(marked),
            Ok(2),
            "the same ring and sector with the mark is another picture"
        );
    }

    #[test]
    fn disk_is_part_of_the_key() {
        let with_disk = IconPicture {
            disk: Some(Disk {
                step: 3,
                blue_twelfths: 6,
            }),
            ..key(8, Some(2))
        };
        let mut f = setup();
        assert_eq!(f.get(key(8, Some(2))), Ok(1));
        assert_eq!(
            f.get(with_disk),
            Ok(2),
            "the same ring and sector with a disk is another picture"
        );
    }

    #[test]
    fn build_gets_the_key() {
        let mut cache = FrameCache::<u32>::new();
        let mut seen = None;
        let wanted = key(6, Some(4));
        let result = cache.get_or_build(wanted, |picture| {
            seen = Some(picture);
            Ok(7)
        });
        assert_eq!(result, Ok(7));
        assert_eq!(seen, Some(wanted));
    }
}

mod ring_change {
    use super::*;

    #[test]
    fn clears_cache() {
        let mut f = setup();
        assert_eq!(f.get(key(8, Some(0))), Ok(1));
        assert_eq!(f.get(key(5, Some(0))), Ok(2), "another ring size");
        assert_eq!(
            f.get(key(8, Some(0))),
            Ok(3),
            "the frame of the old ring was dropped"
        );
    }

    #[test]
    fn frames_of_the_new_ring_are_cached() {
        let mut f = setup();
        f.get(key(8, Some(0))).unwrap();
        assert_eq!(f.get(key(5, Some(0))), Ok(2));
        assert_eq!(
            f.get(key(5, Some(0))),
            Ok(2),
            "the new ring is cached from its first frame"
        );
        assert_eq!(f.builds(), 2);
    }

    #[test]
    fn other_role_of_the_same_size_is_another_ring() {
        let online = IconPicture {
            ring: Some(Ring::uniform(RingRole::Online, 8)),
            ..key(8, Some(0))
        };
        let mut f = setup();
        assert_eq!(f.get(key(8, Some(0))), Ok(1));
        assert_eq!(f.get(online), Ok(2));
        assert_eq!(
            f.get(key(8, Some(0))),
            Ok(3),
            "the grey frame is not served for the green ring"
        );
    }
}

mod build_error {
    use super::*;

    #[test]
    fn is_returned_as_is() {
        let mut f = setup();
        assert_eq!(
            f.get_failing(key(8, Some(0)), "bad raster"),
            Err("bad raster".to_string())
        );
    }

    #[test]
    fn is_not_cached() {
        let mut f = setup();
        f.get_failing(key(8, Some(0)), "bad raster").unwrap_err();

        assert_eq!(
            f.get(key(8, Some(0))),
            Ok(2),
            "the next call tries to build again"
        );
        assert_eq!(f.get(key(8, Some(0))), Ok(2), "and the success is cached");
        assert_eq!(f.builds(), 2);
    }
}

/// A cache plus a counter of `build` calls.
struct Fixture {
    cache: FrameCache<u32>,
    calls: Cell<u32>,
}

impl Fixture {
    /// Asks for the frame; a build returns the number of the build call.
    fn get(&mut self, key: IconPicture) -> Result<u32, String> {
        let calls = &self.calls;
        self.cache.get_or_build(key, |_| {
            calls.set(calls.get() + 1);
            Ok(calls.get())
        })
    }

    /// Asks for the frame with a `build` that fails (and counts as a build call).
    fn get_failing(&mut self, key: IconPicture, reason: &str) -> Result<u32, String> {
        let calls = &self.calls;
        self.cache.get_or_build(key, |_| {
            calls.set(calls.get() + 1);
            Err(reason.to_string())
        })
    }

    /// How many times `build` was called so far.
    fn builds(&self) -> u32 {
        self.calls.get()
    }
}

fn setup() -> Fixture {
    Fixture {
        cache: FrameCache::new(),
        calls: Cell::new(0),
    }
}

/// A grey idle ring of `sectors` sectors with the white sector `lit`.
fn key(sectors: u16, lit: Option<u16>) -> IconPicture {
    IconPicture {
        color: TrayColor::Grey,
        ring: Some(Ring::uniform(RingRole::Offline, sectors)),
        disk: None,
        alert: false,
        lit,
    }
}
