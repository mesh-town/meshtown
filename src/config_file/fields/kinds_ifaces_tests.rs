//! The `IfaceNames` kind: three field states, rejections, a parse-render round trip.

use serde_json::{json, Value};

use super::IfaceNames;
use crate::config_file::fields::value::Kind;
use crate::config_file::IfaceFilter;

#[test]
fn parse_three_states() {
    assert_eq!(IfaceNames.parse(&json!([])), Some(IfaceFilter::Auto));
    assert_eq!(IfaceNames.parse(&Value::Null), Some(IfaceFilter::Auto));
    assert_eq!(IfaceNames.parse(&json!(["*"])), Some(IfaceFilter::Any));
    assert_eq!(
        IfaceNames.parse(&json!(["br-lan"])),
        Some(IfaceFilter::Only(vec!["br-lan".to_string()]))
    );
}

#[test]
fn parse_rejects_wrong_shapes() {
    assert_eq!(IfaceNames.parse(&json!("br-lan")), None);
    assert_eq!(IfaceNames.parse(&json!([1])), None);
    assert_eq!(IfaceNames.parse(&json!(["*", "br-lan"])), None);
}

#[test]
fn to_json_round_trips() {
    for f in [IfaceFilter::Auto, IfaceFilter::Any, IfaceFilter::Only(vec!["a".into(), "b c".into()])] {
        let j = IfaceNames.to_json(&f).expect("the key is written");
        assert_eq!(IfaceNames.parse(&j), Some(f));
    }
}

#[test]
fn texts() {
    assert_eq!(IfaceNames.default_text(&IfaceFilter::Auto), "[]");
    let e = IfaceNames.expected();
    assert!(e.contains("[]") && e.contains("[\"*\"]"), "{e}");
}
