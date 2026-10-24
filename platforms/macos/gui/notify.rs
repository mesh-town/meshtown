//! System notifications on macOS through `osascript` (`display notification`).
//!
//! The texts travel as script arguments after `--`, never inside the script itself.

use meshtown_core::gui::tray::run_notifier;

const PROGRAM: &str = "osascript";

/// Shows a banner; blocks until `osascript` exits.
pub(crate) fn show(title: &str, body: &str) -> Result<(), &'static str> {
    run_notifier(PROGRAM, &args(title, body), &[])
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
