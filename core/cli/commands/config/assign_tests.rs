use serde_json::json;

use super::*;

#[test]
fn splits_at_first_equals() {
    let set = parse_assignment("node.name=a=b").unwrap();
    assert_eq!(set.path(), "node.name");
    let mut s = crate::config::config_file::Settings::default();
    set.apply_to(&mut s);
    assert_eq!(s.net.node_name.as_deref(), Some("a=b"));
}

#[test]
fn no_equals_refused() {
    assert!(matches!(
        parse_assignment("node.name"),
        Err(SetRefusal::Unknown(_))
    ));
    assert!(matches!(
        parse_assignment("=1"),
        Err(SetRefusal::Unknown(_))
    ));
}

#[test]
fn empty_value_is_empty_string() {
    assert_eq!(value_of(""), json!(""));
}

#[test]
fn shown_round_trips_value_of() {
    for raw in ["abc", "\"true\"", "[1]", "null", "20", "true"] {
        let v = value_of(raw);
        assert_eq!(value_of(&shown(&v)), v, "{raw}");
    }
    assert_eq!(shown(&json!("abc")), "abc");
    assert_eq!(shown(&json!("true")), "\"true\"");
}
