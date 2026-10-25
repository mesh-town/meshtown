use super::*;

#[test]
fn text_lists_paths_without_the_config_prefix() {
    let text = retired_text("", &["config.transport.mode".to_string()]);
    assert_eq!(
        text,
        "deprecated fields ignored and dropped on the next write: transport.mode"
    );
}

#[test]
fn text_starts_with_the_scope() {
    let text = retired_text("net slot ab: ", &["config.transport.mode".to_string()]);
    assert!(text.starts_with("net slot ab: deprecated fields"), "{text}");
}
