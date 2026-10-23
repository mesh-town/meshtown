//! Tests of the log line rendering: JSON for `/api/log` and text for `/api/log/download`.

use std::sync::Arc;

use super::{line_json, line_text};
use crate::web_gui::LogLine;

fn line(net: Option<&str>, shown: bool) -> LogLine {
    LogLine {
        seq: 3,
        t: 1.25,
        tag: "tag".into(),
        text: "hello".into(),
        net: net.map(Arc::<str>::from),
        shown,
    }
}

#[test]
fn json_without_label_is_the_plain_four_fields() {
    let v = line_json(&line(None, false));
    assert_eq!(
        v.to_string(),
        r#"{"seq":3,"t":1.3,"tag":"tag","text":"hello"}"#
    );
}

#[test]
fn json_with_stored_but_hidden_label_has_no_net() {
    let v = line_json(&line(Some("main"), false));
    assert!(v.get("net").is_none());
}

#[test]
fn json_with_shown_label_has_net() {
    let v = line_json(&line(Some("main"), true));
    assert_eq!(v["net"], "main");
}

#[test]
fn text_without_shown_label_is_unchanged() {
    assert_eq!(
        line_text(&line(Some("main"), false)),
        "+    1.2s [tag] hello\n"
    );
    assert_eq!(line_text(&line(None, false)), "+    1.2s [tag] hello\n");
}

#[test]
fn text_with_shown_label_has_prefix_before_the_text() {
    assert_eq!(
        line_text(&line(Some("ab2c3d"), true)),
        "+    1.2s [tag] [ab2c3d] hello\n"
    );
}
