//! Writing a string to the OS clipboard with an external program. The platform backend supplies the list of
//! tools for its OS; the string goes only to the tool's stdin (not argv, not the log); the failure reason is
//! a ready-made constant from `app::tray_texts`, without the string's text.

use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};

use crate::gui::tray::app::tray_texts::{REASON_CLIP_FAILED, REASON_NO_TOOL};

/// A program and its arguments.
pub type ClipboardTool = (&'static str, &'static [&'static str]);

/// Tries the tools in turn; the first one that accepts is `Ok`. None found — `REASON_NO_TOOL`,
/// otherwise `REASON_CLIP_FAILED`.
pub fn copy_via(tools: &[ClipboardTool], text: &str) -> Result<(), &'static str> {
    let mut all_missing = true;
    for tool in tools {
        match run_tool(tool, text) {
            Ok(()) => return Ok(()),
            Err(e) => all_missing &= e.kind() == ErrorKind::NotFound,
        }
    }
    Err(if all_missing {
        REASON_NO_TOOL
    } else {
        REASON_CLIP_FAILED
    })
}

/// One tool: the string to stdin, output to null, wait for the exit code.
fn run_tool(tool: &ClipboardTool, text: &str) -> std::io::Result<()> {
    let mut child = tool_command(tool).spawn()?;
    let written = match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(text.as_bytes()),
        None => Ok(()),
    };
    let status = child.wait()?;
    written?;
    match status.success() {
        true => Ok(()),
        false => Err(ErrorKind::Other.into()),
    }
}

fn tool_command(tool: &ClipboardTool) -> Command {
    let mut cmd = Command::new(tool.0);
    cmd.args(tool.1)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
