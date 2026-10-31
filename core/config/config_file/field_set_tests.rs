use serde_json::json;

use super::*;

fn applied(path: &str, value: Value) -> Settings {
    let mut s = Settings::default();
    FieldSet::check(path, value).unwrap().apply_to(&mut s);
    s
}

#[test]
fn json_literal_accepted() {
    assert_eq!(
        applied("probe.interval_sec", json!(20))
            .net
            .probe_interval_sec,
        20
    );
}

#[test]
fn non_json_is_string() {
    let s = applied("checks.doh_server", json!("9.9.9.9:443"));
    assert_eq!(s.net.doh_server, "9.9.9.9:443".parse().unwrap());
}

#[test]
fn number_for_name_refused() {
    assert!(matches!(
        FieldSet::check("node.name", json!(123)),
        Err(SetRefusal::BadValue { .. })
    ));
    assert!(FieldSet::check("node.name", json!("123")).is_ok());
}

#[test]
fn unknown_and_short_name_refused() {
    assert_eq!(
        FieldSet::check("probe.interval", json!(20)).unwrap_err(),
        SetRefusal::Unknown("probe.interval".into())
    );
}

#[test]
fn read_only_and_computed_refused() {
    for path in ["web.port", "network.label"] {
        assert_eq!(
            FieldSet::check(path, json!(1)).unwrap_err(),
            SetRefusal::ReadOnly(field_by_path(path).unwrap().field_path)
        );
    }
}

#[test]
fn network_key_refused() {
    assert_eq!(
        FieldSet::check("network.key", json!("x")).unwrap_err(),
        SetRefusal::NetworkKey
    );
}

#[test]
fn out_of_range_names_bounds() {
    match FieldSet::check("probe.interval_sec", json!(3)) {
        Err(SetRefusal::BadValue { path, expected }) => {
            assert_eq!(path, "probe.interval_sec");
            assert!(
                expected.contains("5") && expected.contains("86400"),
                "{expected}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn file_only_allowed() {
    let set = FieldSet::check("probe.full_cycle", json!(true)).unwrap();
    assert!(set.is_file_only());
    assert!(!FieldSet::check("probe.interval_sec", json!(20))
        .unwrap()
        .is_file_only());
}

#[test]
fn null_for_optional_addr() {
    let mut s = Settings::default();
    s.net.provider_dns = Some("1.1.1.1:53".parse().unwrap());
    FieldSet::check("checks.provider_dns", Value::Null)
        .unwrap()
        .apply_to(&mut s);
    assert_eq!(s.net.provider_dns, None);
}
