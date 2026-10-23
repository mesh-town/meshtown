use super::*;

#[test]
fn host_candidates_match_os() {
    let names: Vec<&str> = TOOLS.iter().map(|t| t.0).collect();
    assert_eq!(names, ["wl-copy", "xclip", "xsel"]);
    assert_eq!(TOOLS[1].1, ["-selection", "clipboard"]);
    assert_eq!(TOOLS[2].1, ["--clipboard", "--input"]);
}
