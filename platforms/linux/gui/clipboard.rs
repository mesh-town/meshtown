//! The clipboard tools of this OS in order of attempts.

use meshtown_core::gui::tray::ClipboardTool;

pub(crate) const TOOLS: &[ClipboardTool] = &[
    ("wl-copy", &[]),
    ("xclip", &["-selection", "clipboard"]),
    ("xsel", &["--clipboard", "--input"]),
];

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
