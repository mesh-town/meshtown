use super::*;

// ==============================================================================================
// classify_* (6)
// ==============================================================================================

#[test]
fn classify_202_is_accepted() {
    assert!(matches!(reply_from_response(202, b""), Reply::Accepted));
}

#[test]
fn classify_401_is_unauthorized() {
    let body = br#"{"error":"bad_token","message":"no token"}"#;
    assert!(matches!(reply_from_response(401, body), Reply::Unauthorized));
}

#[test]
fn classify_503_stopping_is_stopping() {
    let body = br#"{"error":"stopping","message":"node is stopping"}"#;
    assert!(matches!(reply_from_response(503, body), Reply::Stopping));
}

#[test]
fn classify_503_other_is_other() {
    let body = br#"{"error":"bind_failed","message":"port busy"}"#;
    assert!(matches!(reply_from_response(503, body), Reply::OtherStatus(503)));
}

#[test]
fn classify_403_is_other() {
    let body = br#"{"error":"bad_host","message":"wrong Host"}"#;
    assert!(matches!(reply_from_response(403, body), Reply::OtherStatus(403)));
}

#[test]
fn classify_200_non_json_is_other() {
    assert!(matches!(reply_from_response(200, b"not json"), Reply::OtherStatus(200)));
}

#[test]
fn classify_503_non_object_error_is_other() {
    assert!(matches!(reply_from_response(503, br#"["stopping"]"#), Reply::OtherStatus(503)));
    assert!(matches!(reply_from_response(503, br#"{"error": 5}"#), Reply::OtherStatus(503)));
}
