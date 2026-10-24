use super::*;

#[test]
fn texts_go_through_env() {
    let env = env("The title", "The body");

    assert!(env.contains(&("MESHTOWN_NOTE_TITLE", "The title")));
    assert!(env.contains(&("MESHTOWN_NOTE_BODY", "The body")));
}

#[test]
fn script_reads_env_and_builds_nodes() {
    for needle in [
        "$env:MESHTOWN_NOTE_TITLE",
        "$env:MESHTOWN_NOTE_BODY",
        "CreateTextNode",
    ] {
        assert!(SCRIPT.contains(needle), "the script lacks {needle}");
    }
}

#[test]
fn script_is_one_line_without_double_quotes() {
    assert!(!SCRIPT.contains('"'), "a double quote breaks argv quoting");
    assert!(!SCRIPT.contains('\n'), "the script must be one line");
}

#[test]
fn args_end_with_the_script() {
    assert_eq!(ARGS.last(), Some(&SCRIPT));
}
