//! System notifications on Windows through a WinRT toast raised by `powershell.exe`.
//!
//! The texts reach the script through environment variables, never through its command line.

use meshtown_core::gui::tray::run_notifier;

const PROGRAM: &str = "powershell.exe";

const TITLE_VAR: &str = "MESHTOWN_NOTE_TITLE";
const BODY_VAR: &str = "MESHTOWN_NOTE_BODY";

/// One line, no double quotes: PowerShell strings are single-quoted so argv quoting leaves them alone.
const SCRIPT: &str = concat!(
    "$ErrorActionPreference='Stop';",
    "[void][Windows.UI.Notifications.ToastNotificationManager,Windows.UI.Notifications,ContentType=WindowsRuntime];",
    "[void][Windows.Data.Xml.Dom.XmlDocument,Windows.Data.Xml.Dom.XmlDocument,ContentType=WindowsRuntime];",
    "$xml=[Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent('ToastText02');",
    "$lines=$xml.GetElementsByTagName('text');",
    "[void]$lines.Item(0).AppendChild($xml.CreateTextNode($env:MESHTOWN_NOTE_TITLE));",
    "[void]$lines.Item(1).AppendChild($xml.CreateTextNode($env:MESHTOWN_NOTE_BODY));",
    "$toast=[Windows.UI.Notifications.ToastNotification]::new($xml);",
    "$id='{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe';",
    "[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($id).Show($toast)"
);

const ARGS: [&str; 6] = [
    "-NoProfile",
    "-NonInteractive",
    "-WindowStyle",
    "Hidden",
    "-Command",
    SCRIPT,
];

/// Shows a toast; blocks until PowerShell exits.
pub(crate) fn show(title: &str, body: &str) -> Result<(), &'static str> {
    run_notifier(PROGRAM, &ARGS, &env(title, body))
}

/// The environment carrying the texts into the script.
pub(crate) fn env<'a>(title: &'a str, body: &'a str) -> [(&'static str, &'a str); 2] {
    [(TITLE_VAR, title), (BODY_VAR, body)]
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
