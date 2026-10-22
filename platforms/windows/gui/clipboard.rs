//! The clipboard tools of this OS in order of attempts.

use meshtown_core::gui::tray::ClipboardTool;

pub(crate) const TOOLS: &[ClipboardTool] = &[("clip", &[])];

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
