//! Tests of writing to the clipboard (`clipboard.rs`): the list of tools per OS, moving on to the next
//! tool, the string only through stdin.

use super::*;
#[cfg(unix)]
use crate::app::tray_texts::REASON_CLIP_FAILED;
use crate::app::tray_texts::REASON_NO_TOOL;

#[test]
fn host_candidates_match_os() {
    let names: Vec<&str> = clipboard_tools().iter().map(|t| t.0).collect();
    #[cfg(target_os = "macos")]
    assert_eq!(names, ["pbcopy"]);
    #[cfg(target_os = "windows")]
    assert_eq!(names, ["clip"]);
    #[cfg(target_os = "linux")]
    assert_eq!(names, ["wl-copy", "xclip", "xsel"]);
    #[cfg(target_os = "linux")]
    {
        assert_eq!(clipboard_tools()[1].1, ["-selection", "clipboard"]);
        assert_eq!(clipboard_tools()[2].1, ["--clipboard", "--input"]);
    }
}

#[test]
fn missing_tool_is_no_tool() {
    let tools: &[ClipboardTool] = &[("meshtown-no-such-clip-tool", &[])];
    assert_eq!(copy_via(tools, "x"), Err(REASON_NO_TOOL));
}

#[cfg(unix)]
#[test]
fn failing_tool_is_clip_failed() {
    let tools: &[ClipboardTool] = &[("false", &[])];
    assert_eq!(copy_via(tools, "x"), Err(REASON_CLIP_FAILED));
}

#[cfg(unix)]
#[test]
fn falls_through_to_next_tool() {
    let tools: &[ClipboardTool] = &[("meshtown-no-such-clip-tool", &[]), ("cat", &[])];
    assert_eq!(copy_via(tools, "x"), Ok(()));
}

#[cfg(unix)]
#[test]
fn stdin_carries_text() {
    let path = std::env::temp_dir().join(format!("mesh-clip-test-{}", std::process::id()));
    let path_str: &'static str = Box::leak(path.to_string_lossy().into_owned().into_boxed_str());
    let args: &'static [&'static str] = Box::leak(Box::new(["-c", "cat > \"$0\"", path_str]));
    let tools: &[ClipboardTool] = &[("sh", args)];
    assert_eq!(copy_via(tools, "mesh-invite2-abc"), Ok(()));
    let got = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(got, "mesh-invite2-abc");
}
