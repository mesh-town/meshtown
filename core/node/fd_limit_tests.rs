//! Tests for choosing the open-files limit target (see `fd_limit.rs`).

use super::target_for;

const INF: u64 = u64::MAX;

#[test]
fn macos_cap_from_256_and_infinite_hard() {
    assert_eq!(target_for(256, INF, 10240), Some(10240));
}

#[test]
fn hard_below_cap_wins() {
    assert_eq!(target_for(1024, 4096, 65536), Some(4096));
}

#[test]
fn soft_at_or_above_target_is_none() {
    assert_eq!(target_for(10240, INF, 10240), None);
    assert_eq!(target_for(20000, INF, 10240), None);
    assert_eq!(target_for(4096, 4096, 65536), None);
}

fn journal_text() -> String {
    format!("{:?}", crate::node::output::log_page(0, 100_000).lines)
}

#[test]
fn report_writes_only_some() {
    super::report(Some("fd-limit-test-marker-some".to_string()));
    super::report(None);
    let text = journal_text();
    assert!(
        text.contains("fd-limit-test-marker-some"),
        "journal: {text}"
    );
    assert_eq!(text.matches("fd-limit-test-marker-some").count(), 1);
}
