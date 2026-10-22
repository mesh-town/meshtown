//! Tests of `view`: the second menu item (`page_text`, `page_enabled`), the `icon_rgba` raster and the node item (`node_item_text`).

use super::*;
use crate::gui::tray::app::tray_texts::{START_TEXT, STOP_TEXT};
use crate::node::output::APP_NAME;

#[test]
fn open_text_matches_state() {
    let view = |page| View {
        color: TrayColor::Green,
        page,
        node: NodeState::Running,
        ring: Ring::EMPTY,
        disk: None,
        net: NetLink::NoNetwork,
    };
    assert_eq!(page_text(view(PageState::Ready)), "Open Mesh Town…");
    assert_eq!(
        page_text(view(PageState::Starting)),
        "Mesh Town page is starting…"
    );
    assert_eq!(
        page_text(view(PageState::Disabled)),
        "Turn the page on in the settings file (web.enabled), then restart"
    );
    assert_eq!(
        page_text(view(PageState::PortBusy(7777))),
        "Close that program — \"Start Mesh Town\" will become available"
    );
    assert_eq!(
        page_text(view(PageState::Unavailable)),
        "Mesh Town page is unavailable"
    );
}

#[test]
fn open_enabled_only_when_ready() {
    assert!(page_enabled(PageState::Ready));
    assert!(!page_enabled(PageState::Starting));
    assert!(!page_enabled(PageState::Disabled));
    assert!(!page_enabled(PageState::PortBusy(80)));
    assert!(!page_enabled(PageState::Unavailable));
}

#[test]
fn icon_rgba_length_and_alpha() {
    let data = icon_rgba(TrayColor::Green);
    assert_eq!(data.len(), (ICON_SIDE_PX * ICON_SIDE_PX * 4) as usize);

    let center_alpha_index = ((17 * ICON_SIDE_PX + 17) * 4 + 3) as usize;
    assert_eq!(data[center_alpha_index], 255);

    let corner_alpha_index = 3usize; // pixel (0,0)
    assert_eq!(data[corner_alpha_index], 0);
}

#[test]
fn icon_rgba_red_is_red() {
    let data = icon_rgba(TrayColor::Red);
    let center_index = ((18 * ICON_SIDE_PX + 18) * 4) as usize;
    assert!(data[center_index] >= 200, "r: {}", data[center_index]);
    assert!(
        data[center_index + 1] <= 80,
        "g: {}",
        data[center_index + 1]
    );
    assert!(
        data[center_index + 2] <= 80,
        "b: {}",
        data[center_index + 2]
    );
    assert_eq!(data[center_index + 3], 255);
}

/// The text and activity of the node item in the tray menu: `node_item_text` gives the text,
/// the activity is separate, `start_enabled(node) || stop_enabled(node)`.
mod node_item_text {
    use super::*;

    fn item(node: NodeState) -> (&'static str, bool) {
        (
            node_item_text(node),
            start_enabled(node) || stop_enabled(node),
        )
    }

    #[test]
    fn start_stop_texts_name_the_app() {
        assert_eq!(START_TEXT, format!("Start {APP_NAME}"));
        assert_eq!(STOP_TEXT, format!("Stop {APP_NAME}"));
    }

    #[test]
    fn absent_offers_start_and_is_active() {
        assert_eq!(item(NodeState::Absent), (START_TEXT, true));
    }

    #[test]
    fn starting_offers_stop_and_is_active() {
        assert_eq!(item(NodeState::Starting), (STOP_TEXT, true));
    }

    #[test]
    fn running_offers_stop_and_is_active() {
        assert_eq!(item(NodeState::Running), (STOP_TEXT, true));
    }

    #[test]
    fn stopping_shows_progress_and_is_inactive() {
        assert_eq!(item(NodeState::Stopping), ("Stopping…", false));
    }

    #[test]
    fn unknown_offers_start_but_is_inactive() {
        // given: before the start decision the text is still START_TEXT, but the item is inactive
        assert_eq!(item(NodeState::Unknown), (START_TEXT, false));
    }
}
