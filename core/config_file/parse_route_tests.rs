use super::*;

#[test]
fn route_exits_default_and_parse() {
    // given: no route section — when parsing — then the default [].
    let p = parse_text(r#"{"config":{"version":1}}"#).expect("parse");
    assert!(
        p.settings.net.route_exits.is_empty(),
        "no route section — the default []"
    );
    assert_eq!(p.field_errors, vec![]);

    // given: two valid ids — the same in the same order.
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let b = NodeSecret::from_bytes(&[2u8; 32]).peer_id();
    let text = format!(r#"{{"config":{{"version":1, "route": {{"exits": ["{a}", "{b}"]}}}}}}"#);
    let p = parse_text(&text).expect("parse");
    assert_eq!(p.settings.net.route_exits, vec![a, b], "the order is kept");
    assert_eq!(p.field_errors, vec![]);
}

#[test]
fn route_exits_rejects_bad_values() {
    // given: route.exits is not a list, a number in the list, or a string that is not an id.
    for bad_exits in [r#""x""#, r#"["x"]"#, r#"[5]"#] {
        let text = format!(r#"{{"config":{{"version":1, "route": {{"exits": {bad_exits}}}}}}}"#);
        let p = parse_text(&text).expect("parse");
        assert!(
            p.settings.net.route_exits.is_empty(),
            "{bad_exits}: a damaged field — the default []"
        );
        assert_eq!(p.field_errors.len(), 1, "{bad_exits}");
        assert_eq!(p.field_errors[0].field_path, "route.exits", "{bad_exits}");
    }

    // given: a duplicate of the same id.
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let text = format!(r#"{{"config":{{"version":1, "route": {{"exits": ["{a}", "{a}"]}}}}}}"#);
    let p = parse_text(&text).expect("parse");
    assert!(p.settings.net.route_exits.is_empty(), "a duplicate — the default []");
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "route.exits");

    // given: 65 entries — one more than the limit.
    let many: Vec<String> = (0..65u8)
        .map(|i| {
            NodeSecret::from_bytes(&[i.wrapping_add(10); 32])
                .peer_id()
                .to_string()
        })
        .collect();
    let text = format!(
        r#"{{"config":{{"version":1, "route": {{"exits": {}}}}}}}"#,
        serde_json::to_string(&many).expect("serializing the test id list")
    );
    let p = parse_text(&text).expect("parse");
    assert!(
        p.settings.net.route_exits.is_empty(),
        "more than 64 entries — the default []"
    );
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "route.exits");
}

/// The old key `route.all_traffic` in the file is an unknown field (docs/CONFIG-FILE.md section 8), not an error.
#[test]
fn route_all_traffic_key_becomes_unknown() {
    let text = r#"{"config":{"version":1, "route": {"all_traffic": true}}}"#;
    let mut p = parse_text(text).expect("parse");
    p.unknown_paths.sort();
    assert_eq!(p.unknown_paths, vec!["config.route.all_traffic"]);
    assert_eq!(p.field_errors, vec![], "an unknown field is not an error");
}
