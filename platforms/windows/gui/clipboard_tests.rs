use super::*;

#[test]
fn host_candidates_match_os() {
    let names: Vec<&str> = TOOLS.iter().map(|t| t.0).collect();
    assert_eq!(names, ["clip"]);
}
