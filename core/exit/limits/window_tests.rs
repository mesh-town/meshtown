//! Tests of the exit stream window (`window.rs`): how the setting and the memory tier combine into bytes,
//! and the holder that lets the setting change while the node runs.

use super::*;

mod window_bytes {
    use super::*;

    #[test]
    fn zero_setting_takes_the_tier() {
        // given: the setting 0 (by machine memory)
        // when: the tier says 128 KiB, and when it says none
        let small = window_bytes(0, 128);
        let none = window_bytes(0, 0);

        // then: the tier's window in bytes, and no window
        assert_eq!(small, 131_072);
        assert_eq!(none, 0, "tier 0 means no window");
    }

    #[test]
    fn below_the_floor_rises_to_the_floor() {
        // given: settings under the 64 KiB floor, on a tier without a window
        for setting in [1, 63] {
            // when: the window is computed
            let bytes = window_bytes(setting, 0);

            // then: it is the floor, not the tiny value asked for
            assert_eq!(bytes, 65_536, "setting {setting} KiB");
        }
    }

    #[test]
    fn floor_and_above_are_kept() {
        // given: settings at the floor, just above it and at the top of the range
        let cases = [(64, 65_536), (65, 66_560), (65_536, 67_108_864)];
        for (setting, expected) in cases {
            // when: the window is computed
            let bytes = window_bytes(setting, 128);

            // then: the setting in bytes, unchanged
            assert_eq!(bytes, expected, "setting {setting} KiB");
        }
    }

    #[test]
    fn explicit_setting_wins_over_the_tier() {
        // given: a setting of 256 KiB
        for tier in [0, 128] {
            // when: the tier has no window, and when it has a smaller one
            let bytes = window_bytes(256, tier);

            // then: the setting decides
            assert_eq!(bytes, 262_144, "tier {tier} KiB");
        }
    }
}

mod stream_window {
    use super::*;

    #[test]
    fn starts_off() {
        // given: a window holder nobody configured
        let window = StreamWindow::default();

        // then: there is no window
        assert_eq!(window.bytes(), 0);
    }

    #[test]
    fn configure_sets_setting_and_tier() {
        // given: a window holder
        let window = StreamWindow::default();

        // when: configured with the setting 0 and a tier of 128 KiB
        window.configure(0, 128);

        // then: the tier decides
        assert_eq!(window.bytes(), 131_072);

        // when: configured with a setting of 256 KiB
        window.configure(256, 128);

        // then: the setting decides
        assert_eq!(window.bytes(), 262_144);
    }

    #[test]
    fn set_changes_only_the_setting() {
        // given: a holder configured with the setting 0 and a tier of 128 KiB
        let window = StreamWindow::default();
        window.configure(0, 128);

        // when: the setting becomes 256 KiB
        window.set(256);

        // then: it wins over the tier
        assert_eq!(window.bytes(), 262_144);

        // when: the setting goes back to 0
        window.set(0);

        // then: the tier is still there and takes over
        assert_eq!(
            window.bytes(),
            131_072,
            "the tier survives a change of the setting"
        );
    }
}
