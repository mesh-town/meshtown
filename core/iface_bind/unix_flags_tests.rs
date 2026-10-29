use super::*;
use crate::iface_bind::test_support::{loopback_name, MISSING_IFACE};
use crate::iface_bind::IfaceState;

mod state_from {
    use super::*;

    // given an interface with every flag set, when classified, then it is up
    #[test]
    fn up_and_running_is_up() {
        assert_eq!(state_from(true, true, true), IfaceState::Up);
    }

    // given a found interface lacking up or running, when classified, then it is down
    #[test]
    fn down_or_not_running_is_down() {
        let cases = [(false, false), (false, true), (true, false)];
        for (up, running) in cases {
            assert_eq!(
                state_from(true, up, running),
                IfaceState::Down,
                "found, up={up}, running={running}"
            );
        }
    }

    // given no entry with the name, when classified, then it is missing whatever the flags
    #[test]
    fn not_found_is_missing() {
        for (up, running) in [(false, false), (false, true), (true, false), (true, true)] {
            assert_eq!(
                state_from(false, up, running),
                IfaceState::Missing,
                "not found, up={up}, running={running}"
            );
        }
    }
}

mod flags_state {
    use super::*;

    // given the loopback interface, when its flags are read, then it is up
    #[test]
    fn loopback_is_up() {
        assert_eq!(flags_state(&loopback_name()), IfaceState::Up);
    }

    // given a name no machine has, when flags are read, then it is missing
    #[test]
    fn unknown_name_is_missing() {
        assert_eq!(flags_state(MISSING_IFACE), IfaceState::Missing);
    }
}
