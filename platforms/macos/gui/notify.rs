//! System notifications on macOS. Inside an `.app` bundle they go through `UNUserNotificationCenter` (`notify_un`);
//! a bare binary has no bundle identity, so it falls back to `osascript` (`display notification`).
//!
//! The `osascript` texts travel as script arguments after `--`, never inside the script itself.

use meshtown_core::gui::tray::run_notifier;
use objc2_foundation::NSBundle;

const PROGRAM: &str = "osascript";

/// Shows a banner; blocks until the notification is handed to the system (bundle) or `osascript` exits.
pub(crate) fn show(title: &str, body: &str) -> Result<(), &'static str> {
    let bundle = NSBundle::mainBundle();
    let path = bundle.bundlePath().to_string();
    let id = bundle.bundleIdentifier().map(|s| s.to_string());
    if in_bundle(&path, id.as_deref()) {
        return super::notify_un::show(title, body);
    }
    run_notifier(PROGRAM, &args(title, body), &[])
}

/// Whether the process runs from an `.app` bundle with a non-empty identifier — the condition for the notification center.
pub(crate) fn in_bundle(path: &str, id: Option<&str>) -> bool {
    path.ends_with(".app") && id.is_some_and(|id| !id.is_empty())
}

/// The `osascript` arguments: a three-line script reading the texts from `argv`, then the texts.
pub(crate) fn args<'a>(title: &'a str, body: &'a str) -> Vec<&'a str> {
    vec![
        "-e",
        "on run argv",
        "-e",
        "display notification (item 2 of argv) with title (item 1 of argv)",
        "-e",
        "end run",
        "--",
        title,
        body,
    ]
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
