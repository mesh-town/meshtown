//! System notifications on Linux through `notify-send`.
//!
//! The texts are plain arguments after `--`, so a leading dash is not read as an option.

use meshtown_core::gui::tray::{run_notifier, APP_NAME};

const PROGRAM: &str = "notify-send";

/// Shows a notification; blocks until `notify-send` exits.
pub(crate) fn show(title: &str, body: &str) -> Result<(), &'static str> {
    run_notifier(PROGRAM, &args(title, body), &[])
}

/// The `notify-send` arguments: the application name, `--`, the title and the body.
pub(crate) fn args<'a>(title: &'a str, body: &'a str) -> Vec<&'a str> {
    vec!["--app-name", APP_NAME, "--", title, body]
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
