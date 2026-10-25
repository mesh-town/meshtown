//! The "icon is hidden" check (the sign is that the `NSStatusItem` button window is not visible and is parked below the bottom
//! edge of the screen) and a window with an "Open settings" button. There is no timer: the delegate calls `check` once after
//! launch and, if the icon looked hidden, `recheck` a few seconds later, both through `performSelector:…afterDelay:`.
//! The window is shown only when both checks saw a hidden icon (the menu bar can still be loading on a cold start).

use std::cell::Cell;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSApplication, NSStatusItem, NSWindowOcclusionState,
};
use objc2_foundation::NSString;

use meshtown_core::gui::tray::{
    hidden_icon_hint_text, hidden_icon_title_text, log_tray, HIDDEN_ICON_LATER_TEXT,
    HIDDEN_ICON_OPEN_SETTINGS_TEXT,
};

/// Delay before the first check after `attach_ui`.
pub(crate) const CHECK_DELAY_SECS: f64 = 2.0;
/// Delay between a first "hidden" verdict and the second check that confirms it.
pub(crate) const RECHECK_DELAY_SECS: f64 = 5.0;
pub(crate) const MENU_BAR_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.ControlCenter-Settings.extension";
/// The top edge of the window frame (`maxY`) is no higher than this (in points) — the window is wholly below the bottom edge of the
/// screen. A hidden icon on macOS 26 keeps the window at `(0, -22)` with height 22, i.e. `maxY` is exactly 0;
/// a visible one is above 600.
const PARKED_MAX_Y: f64 = 1.0;

/// What the process sees about the icon button's window: the "visible" bit from `occlusionState` and the top edge of the
/// frame (`frame.maxY`, y axis up from the bottom of the main screen).
#[derive(Clone, Copy, Debug, PartialEq)]
struct IconWindow {
    visible: bool,
    max_y: f64,
}

/// Whether the icon is hidden: the "visible" bit is cleared AND the window is parked below the bottom edge of the screen. One sign
/// is not enough: `NSStatusItem.isVisible` and `NSWindow.isVisible` are always `true`; the "visible" bit is also cleared
/// when the system itself hides the menu bar (a full-screen window, auto-hide) — there the frame stays
/// at the top edge. Unknown (no window) — not hidden, we show no window.
fn looks_hidden(window: Option<IconWindow>) -> bool {
    match window {
        Some(w) => !w.visible && w.max_y <= PARKED_MAX_Y,
        None => false,
    }
}

/// The icon button's window as `IconWindow`; a missing link (button or window) — `None`.
fn icon_window(item: &NSStatusItem, mtm: MainThreadMarker) -> Option<IconWindow> {
    let window = item.button(mtm)?.window()?;
    let frame = window.frame();
    Some(IconWindow {
        visible: window
            .occlusionState()
            .contains(NSWindowOcclusionState::Visible),
        max_y: frame.origin.y + frame.size.height,
    })
}

/// The icon counts as hidden only when both checks saw it hidden; a visible or unknown window in either one cancels the alert.
fn confirmed_hidden(first: Option<IconWindow>, second: Option<IconWindow>) -> bool {
    looks_hidden(first) && looks_hidden(second)
}

thread_local! {
    /// The window seen by the first check, handed to the second one (both run on the main thread).
    static FIRST: Cell<Option<IconWindow>> = const { Cell::new(None) };
}

/// Samples the icon window and writes the verdict to the acceptance log under `label`.
fn sample(item: &NSStatusItem, mtm: MainThreadMarker, label: &str) -> Option<IconWindow> {
    let window = icon_window(item, mtm);
    let hidden = looks_hidden(window);
    log_tray(&format!(
        "icon: window {window:?}, hidden: {hidden} ({label})"
    ));
    window
}

/// The first "hidden" check; returns `true` when the icon looks hidden and `recheck` should be scheduled.
pub(crate) fn check(item: &NSStatusItem, mtm: MainThreadMarker) -> bool {
    let window = sample(item, mtm, "first");
    FIRST.with(|cell| cell.set(window));
    looks_hidden(window)
}

/// The second check: the alert opens only if `confirmed_hidden` holds for both samples.
pub(crate) fn recheck(item: &NSStatusItem, mtm: MainThreadMarker) {
    let second = sample(item, mtm, "second");
    if confirmed_hidden(FIRST.with(Cell::get), second) {
        show_hidden_icon_alert(mtm);
    }
}

/// An `NSAlert` with the buttons "Open settings"/"Not now"; the first button opens the "Menu
/// bar" panel.
fn show_hidden_icon_alert(mtm: MainThreadMarker) {
    // The replacement for `NSApplication::activate` exists only since macOS 14, the package's `LSMinimumSystemVersion` is
    // 11.0; without activation the window goes under other apps with the Accessory policy.
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);

    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(&hidden_icon_title_text()));
    alert.setInformativeText(&NSString::from_str(&hidden_icon_hint_text()));
    alert.addButtonWithTitle(&NSString::from_str(HIDDEN_ICON_OPEN_SETTINGS_TEXT));
    alert.addButtonWithTitle(&NSString::from_str(HIDDEN_ICON_LATER_TEXT));

    if alert.runModal() != NSAlertFirstButtonReturn {
        return;
    }
    if let Err(e) = crate::open_url::open(MENU_BAR_SETTINGS_URL) {
        log_tray(&format!("settings did not open: {e}"));
    }
}

#[cfg(test)]
#[path = "hidden_icon_tests.rs"]
mod tests;
