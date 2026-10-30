//! Tests of the text of a refusal from the node (`api_error.rs`).

use super::*;

#[test]
fn api_error_text_uses_message() {
    let e = ApiError::from_reply(409, r#"{"error":"not_admin","message":"only admins can do this"}"#);
    assert_eq!(e.code, "not_admin");
    let text = e.to_string();
    assert!(text.starts_with("only admins can do this. "), "{text}");
    assert!(text.contains("administrator"), "{text}");
}

#[test]
fn api_error_without_message_names_status() {
    let e = ApiError::from_reply(500, "not json");
    assert_eq!(e.to_string(), "Mesh Town answered HTTP 500.");
    assert!(e.body.is_null());
}

#[test]
fn refusal_advice_names_next_step() {
    for code in ["not_admin", "self_target", "last_admin", "network_deleted", "too_many_nets"] {
        let advice = refusal_advice(code).unwrap_or_default();
        assert!(advice.ends_with('.') && advice.len() > 10, "{code}: {advice}");
    }
    assert!(refusal_advice("last_admin").unwrap_or_default().contains("member admin"));
}

#[test]
fn unknown_code_has_no_advice() {
    assert!(refusal_advice("something_new").is_none());
    let e = ApiError::from_reply(418, r#"{"error":"something_new","message":"teapot."}"#);
    assert_eq!(e.to_string(), "teapot.");
}

#[test]
fn unauthorized_tells_who_to_run_as() {
    let e = ApiError::from_reply(401, r#"{"message":"unauthorized"}"#);
    assert!(e.to_string().contains("--config"), "{e}");
}
