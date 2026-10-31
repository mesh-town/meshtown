use serde_json::{json, Value};

use super::*;

const A: &str = "aaaa111122223333444455556666777788889999aaaabbbbccccddddeeeeffff";
const B: &str = "aaaa9999888877776666555544443333222211110000ffffeeeeddddccccbbbb";
const C: &str = "beef000000000000000000000000000000000000000000000000000000000001";
const D: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn member(id_full: &str, name: Value, me: bool) -> Value {
    json!({"id": &id_full[..10], "id_full": id_full, "name": name, "self": me, "status": "member"})
}

fn list() -> Value {
    json!([
        member(A, json!("Laptop"), true),
        member(B, json!("Ñandú"), false),
        member(C, json!(null), false),
        member(D, json!(""), false),
    ])
}

#[test]
fn name_ignores_case() {
    assert_eq!(resolve_member(&list(), "laptop"), Ok(A.to_string()));
    assert_eq!(resolve_member(&list(), "  LAPTOP "), Ok(A.to_string()));
    assert_eq!(resolve_member(&list(), "ñandú"), Ok(B.to_string()));
}

#[test]
fn null_and_empty_names_skipped() {
    for wanted in ["-", "\"\"", "null"] {
        let err = resolve_member(&list(), wanted).unwrap_err();
        assert!(err.contains("no member"), "{wanted}: {err}");
    }
}

#[test]
fn duplicate_names_listed() {
    let two = json!([
        member(A, json!("laptop"), true),
        member(B, json!("Laptop"), false)
    ]);
    let err = resolve_member(&two, "laptop").unwrap_err();
    assert!(err.contains("several members are named"), "{err}");
    assert!(
        err.contains("aaaa111122 member laptop (this computer)"),
        "{err}"
    );
    assert!(err.contains("aaaa999988 member Laptop"), "{err}");
    assert!(err.contains("member list"), "{err}");
}

#[test]
fn name_beats_prefix() {
    let rows = json!([
        member(A, json!("laptop"), true),
        member(C, json!("aaaa"), false)
    ]);
    assert_eq!(resolve_member(&rows, "aaaa"), Ok(C.to_string()));
}

#[test]
fn prefix_from_four_digits() {
    assert_eq!(resolve_member(&list(), "beef"), Ok(C.to_string()));
    assert_eq!(resolve_member(&list(), "BEEF000000"), Ok(C.to_string()));
    assert_eq!(resolve_member(&list(), C), Ok(C.to_string()));
    assert_eq!(
        resolve_member(&list(), &D.to_uppercase()),
        Ok(D.to_string())
    );
}

#[test]
fn short_prefix_refused() {
    let err = resolve_member(&list(), "bee").unwrap_err();
    assert!(err.contains("at least 4"), "{err}");
    let three = json!([member(A, json!("pc3"), true)]);
    assert_eq!(resolve_member(&three, "pc3"), Ok(A.to_string()));
}

#[test]
fn ambiguous_prefix_listed() {
    let err = resolve_member(&list(), "aaaa").unwrap_err();
    assert!(
        err.contains("several members have an id starting with aaaa"),
        "{err}"
    );
    assert!(
        err.contains("aaaa111122") && err.contains("aaaa999988"),
        "{err}"
    );
    assert!(err.contains("(this computer)"), "{err}");
}

#[test]
fn unknown_and_foreign_full_id() {
    let foreign = "f".repeat(64);
    for wanted in ["nobody", "dead", foreign.as_str()] {
        let err = resolve_member(&list(), wanted).unwrap_err();
        assert!(err.contains("no member matches"), "{wanted}: {err}");
        assert!(err.contains("member list"), "{err}");
    }
}

#[test]
fn empty_input_and_empty_list() {
    assert!(resolve_member(&list(), "  ").is_err());
    assert!(resolve_member(&json!([]), "laptop").is_err());
    assert!(resolve_member(&Value::Null, "beef").is_err());
}

#[test]
fn row_without_id_full_skipped() {
    let rows = json!([{"id": "aaaa111122", "name": "laptop", "status": "member"}]);
    assert!(resolve_member(&rows, "laptop").is_err());
    assert!(resolve_member(&rows, "aaaa").is_err());
}

#[test]
fn self_resolves() {
    assert_eq!(resolve_member(&list(), "Laptop"), Ok(A.to_string()));
    assert_eq!(resolve_member(&list(), "aaaa1111"), Ok(A.to_string()));
}
