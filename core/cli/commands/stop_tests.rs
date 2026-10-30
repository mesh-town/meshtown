use serde_json::json;

use super::*;

#[test]
fn not_running_means_gone() {
    let answer = Err(anyhow::Error::new(NotRunning { port: 1 }).context("asking"));
    assert!(is_gone(&answer));
}

#[test]
fn an_answer_or_another_error_means_still_there() {
    assert!(!is_gone(&Ok(json!({"nets": []}))));
    assert!(!is_gone(&Err(anyhow::anyhow!("refused"))));
}
