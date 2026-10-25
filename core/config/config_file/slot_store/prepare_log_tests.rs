//! What the log says about a slot file: retired fields get their own warning.

use super::*;
use crate::config::config_file::parse_net_text;
use crate::node::output::{log_page, log_summary};

fn config_lines_for(text: &str) -> Vec<String> {
    let p = parse_net_text(text).expect("the slot file parses");
    let seq_before = log_summary(0).last_seq;
    log_file_findings(&NetSlotId::parse("ab2c3d").unwrap(), None, &p);
    log_page(seq_before, 4000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "config")
        .map(|l| l.text)
        .collect()
}

#[test]
fn retired_mode_in_slot_file_is_logged() {
    // given: a slot file with the retired `transport.mode`
    let text = r#"{"config": {"version": 1, "transport": {"mode": "no_relay"}}}"#;
    // when: the findings are logged
    let lines = config_lines_for(text);
    // then: the warning names the slot and the path; there is no "unknown" line for it
    let want =
        "net slot ab2c3d: deprecated fields ignored and dropped on the next write: transport.mode";
    assert!(lines.iter().any(|l| l == want), "{lines:?}");
    assert!(
        !lines.iter().any(|l| l.contains("unknown fields")),
        "{lines:?}"
    );
}
