//! Tests of the menu state shown when the summary of all networks has no visible network: which slot's state
//! the menu is taken from.

use super::views_from_state_json;
use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::local_api::stub_node::{overview_json, state_json, NetSpec, StateSpec};
use crate::gui::tray::app::proxy_state::ProxyState;

mod main_slot_first {
    use super::*;

    #[test]
    fn menu_state_is_taken_from_the_main_slot() {
        // given
        let slots = [keyless("main", true), keyless("ab12cd", false)];

        // when
        let menu = menu_of(&slots);

        // then
        assert!(menu.serves_exit, "the state of the main slot is shown");
        assert_eq!(menu.slot, None);
    }
}

mod no_main_slot {
    use super::*;

    #[test]
    fn menu_state_is_taken_from_the_added_slot() {
        // no main slot in the summary: the first slot with a state is shown
        // given
        let slots = [keyless("ab12cd", true)];

        // when
        let menu = menu_of(&slots);

        // then
        assert!(menu.serves_exit, "the state of the only slot is shown");
        assert_eq!(menu.slot, None);
    }
}

/// A slot without a key (so it is not visible) whose state may serve as an exit.
fn keyless(id: &'static str, serves_exit: bool) -> NetSpec<'static> {
    let state = StateSpec {
        network: false,
        serves_exit,
        ..Default::default()
    };
    NetSpec {
        state: Some(state_json(&state)),
        ..NetSpec::keyless(id)
    }
}

fn menu_of(nets: &[NetSpec]) -> ExitMenu {
    let plan = ProxyState::On("127.0.0.1:1080".to_string());
    views_from_state_json(&overview_json(nets), &plan).0
}
