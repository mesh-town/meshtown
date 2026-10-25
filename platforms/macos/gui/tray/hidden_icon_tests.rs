use super::*;
use meshtown_core::gui::tray::hidden_icon_hint_text as hint_text;

fn window(visible: bool, max_y: f64) -> Option<IconWindow> {
    Some(IconWindow { visible, max_y })
}

#[test]
fn looks_hidden_parked_and_invisible() {
    // Measurement: the toggle is off — frame (0, -22, 34, 22), the "visible" bit is cleared.
    assert!(looks_hidden(window(false, 0.0)));
}

#[test]
fn looks_hidden_shown_in_menu_bar() {
    // Measurement: the toggle is on — frame (744, 645, 34, 30), the "visible" bit is set.
    assert!(!looks_hidden(window(true, 675.0)));
}

#[test]
fn looks_hidden_system_hides_menu_bar() {
    // Measurement: full-screen Safari — the bit is cleared, the frame is above the screen (744, 675, 34, 30).
    assert!(!looks_hidden(window(false, 705.0)));
}

#[test]
fn looks_hidden_visible_below_screen() {
    // The window is visible (for example, a second screen below the main one) — not hidden, however low it stands.
    assert!(!looks_hidden(window(true, -200.0)));
}

#[test]
fn looks_hidden_unknown() {
    assert!(!looks_hidden(None));
}

#[test]
fn looks_hidden_boundary() {
    assert!(looks_hidden(window(false, 1.0)));
    assert!(!looks_hidden(window(false, 1.1)));
}

#[test]
fn settings_url_scheme() {
    assert!(MENU_BAR_SETTINGS_URL.starts_with("x-apple.systempreferences:"));
}

#[test]
fn hint_text_names_the_product() {
    let expected = format!(
        "Open System Settings and switch {} on in the list of apps allowed in the menu bar.",
        meshtown_core::node::output::APP_NAME
    );
    assert_eq!(hint_text(), expected);
}

#[test]
fn hint_text_names_no_settings_section() {
    let text = hint_text();
    assert!(!text.contains('→'), "{text}");
    assert!(!text.contains("Allow in the Menu Bar"), "{text}");
    assert!(
        text.contains(meshtown_core::node::output::APP_NAME),
        "{text}"
    );
}

#[test]
fn confirmed_hidden_needs_both_checks() {
    assert!(confirmed_hidden(window(false, 0.0), window(false, 0.0)));
}

#[test]
fn confirmed_hidden_first_visible_wins() {
    assert!(!confirmed_hidden(window(true, 675.0), window(false, 0.0)));
    assert!(!confirmed_hidden(window(false, 0.0), window(true, 675.0)));
}

#[test]
fn confirmed_hidden_unknown_second() {
    assert!(!confirmed_hidden(window(false, 0.0), None));
    assert!(!confirmed_hidden(None, window(false, 0.0)));
}
