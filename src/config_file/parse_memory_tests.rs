use super::*;

#[test]
fn memory_guard_defaults_are_400_and_900() {
    let p = parse_text(r#"{"config":{"version":1}}"#).expect("parse");
    assert_eq!(p.settings.process.node_memory_warn_mib, 400);
    assert_eq!(p.settings.process.node_memory_exit_mib, 900);
}

#[test]
fn memory_guard_zero_means_off_and_keys_are_known() {
    let text = r#"{"config":{"version":1,"node":{"memory_warn_mib":0,"memory_exit_mib":0}}}"#;
    let p = parse_text(text).expect("parse");
    assert_eq!(p.settings.process.node_memory_warn_mib, 0);
    assert_eq!(p.settings.process.node_memory_exit_mib, 0);
    assert_eq!(p.field_errors, vec![]);
    assert_eq!(p.unknown_paths, Vec::<String>::new());
}

#[test]
fn memory_exit_below_warn_is_field_error() {
    let text = r#"{"config":{"version":1,"node":{"memory_warn_mib":500,"memory_exit_mib":300}}}"#;
    let p = parse_text(text).expect("parse");
    assert_eq!(p.settings.process.node_memory_warn_mib, 500);
    assert_eq!(p.settings.process.node_memory_exit_mib, 900);
    assert_eq!(p.field_errors.len(), 1);
    assert_eq!(p.field_errors[0].field_path, "node.memory_exit_mib");
}

#[test]
fn memory_warn_above_default_exit_lifts_exit() {
    let text = r#"{"config":{"version":1,"node":{"memory_warn_mib":1000}}}"#;
    let p = parse_text(text).expect("parse");
    assert_eq!(p.settings.process.node_memory_warn_mib, 1000);
    assert_eq!(p.settings.process.node_memory_exit_mib, 1000);
    assert_eq!(p.field_errors, vec![]);
}

#[test]
fn memory_exit_below_big_warn_takes_warn() {
    let text = r#"{"config":{"version":1,"node":{"memory_warn_mib":1000,"memory_exit_mib":300}}}"#;
    let p = parse_text(text).expect("parse");
    assert_eq!(p.settings.process.node_memory_exit_mib, 1000);
    assert_eq!(p.field_errors.len(), 1);
}
