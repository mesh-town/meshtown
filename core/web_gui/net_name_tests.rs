use super::net_display_name;

#[test]
fn label_wins() {
    assert_eq!(
        net_display_name(Some("work"), Some("9c1e5a")).as_deref(),
        Some("work")
    );
}

#[test]
fn empty_label_falls_back_to_the_key() {
    assert_eq!(
        net_display_name(Some(""), Some("9c1e5a")).as_deref(),
        Some("Network 9c1e")
    );
}

#[test]
fn key_shorter_than_four_is_taken_whole() {
    assert_eq!(
        net_display_name(None, Some("ab")).as_deref(),
        Some("Network ab")
    );
}

#[test]
fn key_gives_first_four_characters() {
    assert_eq!(
        net_display_name(None, Some("9c1e5a77")).as_deref(),
        Some("Network 9c1e")
    );
}

#[test]
fn nothing_gives_none() {
    assert_eq!(net_display_name(None, None), None);
    assert_eq!(net_display_name(Some(""), None), None);
}
