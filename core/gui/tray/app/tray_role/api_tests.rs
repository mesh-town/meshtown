//! Tests of the decision policy for the node's reply: spawn, attach, port in use (`api.rs`).

use super::*;

#[test]
fn policy_refused_spawns() {
    assert_eq!(start_policy(&Err(ApiError::Refused)), StartDecision::Spawn);
}

#[test]
fn policy_200_attaches() {
    assert_eq!(
        start_policy(&Ok(Reply::Ok(serde_json::json!({})))),
        StartDecision::Attach
    );
}

#[test]
fn policy_everything_else_is_port_busy() {
    let busy = [
        Ok(Reply::Unauthorized),
        Ok(Reply::Accepted),
        Ok(Reply::Stopping),
        Ok(Reply::OtherStatus(404)),
        Err(ApiError::Timeout),
        Err(ApiError::NotHttp("SSH".to_string())),
    ];
    for reply in &busy {
        assert_eq!(
            start_policy(reply),
            StartDecision::PortBusy,
            "{}",
            reply_summary(reply)
        );
    }
}
