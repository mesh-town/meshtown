use super::*;

mod out_iface {
    use super::*;

    mod new {
        use super::*;

        // given no name, when the handle is built, then it means any network
        #[test]
        fn none_is_any_network() {
            assert!(OutIface::new(None).current().is_none());
        }

        // given a name, when the handle is built, then the name is current
        #[test]
        fn name_is_current() {
            assert_eq!(current_text(&OutIface::new(Some("wg0"))), Some("wg0".into()));
        }
    }

    mod set {
        use super::*;

        // given a handle, when the value changes, then set reports true and then false for a repeat
        #[test]
        fn set_reports_change() {
            let iface = OutIface::default();
            assert!(iface.set(Some("wg0")), "none -> name is a change");
            assert!(!iface.set(Some("wg0")), "the same name is not a change");
            assert!(iface.set(Some("wg1")), "name -> other name is a change");
            assert!(iface.set(None), "name -> none is a change");
            assert!(!iface.set(None), "none -> none is not a change");
        }

        // given a handle, when set is called, then current returns the new value
        #[test]
        fn current_follows_set() {
            let iface = OutIface::new(Some("wg0"));
            iface.set(None);
            assert_eq!(current_text(&iface), None);
            iface.set(Some("wg1"));
            assert_eq!(current_text(&iface), Some("wg1".into()));
        }

        // given a clone of a handle, when one side sets, then the other side sees it
        #[test]
        fn clones_share_the_value() {
            let a = OutIface::default();
            let b = a.clone();
            a.set(Some("wg0"));
            assert_eq!(current_text(&b), Some("wg0".into()));
        }

        // given a poisoned lock, when reading and setting, then both still work
        #[test]
        fn poisoned_lock_is_survived() {
            let iface = OutIface::new(Some("wg0"));
            poison(&iface);
            assert_eq!(current_text(&iface), Some("wg0".into()));
            assert!(iface.set(Some("wg1")), "set works after a panic in another thread");
            assert_eq!(current_text(&iface), Some("wg1".into()));
        }
    }
}

fn current_text(iface: &OutIface) -> Option<String> {
    iface.current().map(|n| n.to_string())
}

/// Panics in another thread while it holds the handle, so the inner lock is poisoned.
fn poison(iface: &OutIface) {
    let other = iface.clone();
    let result = std::thread::spawn(move || {
        let _guard = other.0.write();
        panic!("poison the lock");
    })
    .join();
    assert!(result.is_err(), "helper thread must panic");
}
