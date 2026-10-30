use super::*;
use crate::autostart::fake;

const ALL_STATES: [EntryState; 4] = [
    EntryState::Absent,
    EntryState::SwitchedOff,
    EntryState::Current,
    EntryState::Stale,
];

mod plan_change {
    use super::*;

    #[test]
    fn covers_every_state() {
        // given: every (desired, state) pair with the expected action
        let table = [
            (true, EntryState::Absent, Some(Change::Enable)),
            (true, EntryState::SwitchedOff, Some(Change::Enable)),
            (true, EntryState::Current, None),
            (true, EntryState::Stale, Some(Change::Enable)),
            (false, EntryState::Absent, None),
            (false, EntryState::SwitchedOff, None),
            (false, EntryState::Current, Some(Change::Disable)),
            (false, EntryState::Stale, Some(Change::Disable)),
        ];
        for (desired, state, expected) in table {
            // when / then
            assert_eq!(
                plan_change(desired, state),
                expected,
                "desired={desired} state={state:?}"
            );
        }
    }
}

mod is_on {
    use super::*;

    #[test]
    fn only_current_and_stale_count_as_on() {
        let on: Vec<EntryState> = ALL_STATES.into_iter().filter(|s| s.is_on()).collect();
        assert_eq!(on, [EntryState::Current, EntryState::Stale]);
    }
}

mod fake_backend {
    use super::*;

    #[test]
    fn enable_then_disable_moves_state_and_logs_calls() {
        // given
        let backend = fake::leaked(EntryState::Absent);
        // when
        backend.enable().unwrap();
        let after_enable = backend.state().unwrap();
        backend.disable().unwrap();
        let after_disable = backend.state().unwrap();
        // then
        assert_eq!(after_enable, EntryState::Current);
        assert_eq!(after_disable, EntryState::Absent);
        assert_eq!(backend.calls(), ["enable", "state", "disable", "state"]);
    }

    #[test]
    fn failed_enable_keeps_state_and_reports_error() {
        // given
        let backend = fake::leaked_with(Ok(EntryState::Absent), Err("denied".into()), Ok(()));
        // when
        let result = backend.enable();
        // then
        assert_eq!(result, Err("denied".to_string()));
        assert_eq!(backend.state(), Ok(EntryState::Absent));
    }
}

mod refresh_on_start {
    use super::*;

    #[test]
    fn stale_entry_is_rewritten() {
        let backend = fake::leaked(EntryState::Stale);
        assert_eq!(refresh_on_start(backend), Ok(true));
        assert_eq!(backend.calls(), ["state", "enable"]);
    }

    #[test]
    fn other_states_are_left_alone() {
        for state in [
            EntryState::Absent,
            EntryState::SwitchedOff,
            EntryState::Current,
        ] {
            let backend = fake::leaked(state);
            assert_eq!(refresh_on_start(backend), Ok(false), "{state:?}");
            assert_eq!(backend.calls(), ["state"], "{state:?}");
        }
    }

    #[test]
    fn state_error_skips_enable() {
        let backend = fake::leaked_with(Err("boom".into()), Ok(()), Ok(()));
        assert_eq!(refresh_on_start(backend), Err("boom".to_string()));
        assert_eq!(backend.calls(), ["state"]);
    }

    #[test]
    fn enable_error_is_reported() {
        let backend = fake::leaked_with(Ok(EntryState::Stale), Err("denied".into()), Ok(()));
        assert_eq!(refresh_on_start(backend), Err("denied".to_string()));
        assert_eq!(backend.calls(), ["state", "enable"]);
    }
}
