//! Tests of the unified node output (`output.rs`).

use super::*;

/// `JOURNAL` is a process static: the tests that touch it are serialized so as not to interfere with
/// one another; each restores what it changed before exiting.
static TEST_GUARD: Mutex<()> = Mutex::new(());

fn lock_tests() -> std::sync::MutexGuard<'static, ()> {
    TEST_GUARD.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn journal_page_numbers_lines_and_carries_given_boot_id() {
    let j = Journal::new(10);
    j.say("t", "a");
    j.say("t", "b");

    let first = j.page("b1", 0, 10, None);
    assert_eq!(first.boot_id, "b1");
    assert_eq!(first.lines.len(), 2);
    assert_eq!(first.lines[0].text, "a");
    assert_eq!(first.lines[1].text, "b");
    assert!(first.lines[1].seq > first.lines[0].seq);

    let seq_a = first.lines[0].seq;
    let second = j.page("b2", seq_a, 10, None);
    assert_eq!(second.boot_id, "b2");
    assert_eq!(second.lines.len(), 1);
    assert_eq!(second.lines[0].text, "b");
    assert_eq!(second.lines[0].seq, first.lines[1].seq);
}

#[test]
fn journal_set_capacity_trims_and_summary_reports_it() {
    let j = Journal::new(10);
    j.set_capacity(2);
    j.say("t", "a");
    j.say("t", "b");
    j.say("t", "c");

    let summary = j.summary(10);
    assert_eq!(summary.ring_lines, 2);
    let texts: Vec<_> = summary.tail.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["b", "c"]);
}

#[test]
fn journal_summary_tail_and_last_seq() {
    let j = Journal::new(10);
    j.say("t", "a");
    j.say("t", "b");
    j.say("t", "c");

    assert_eq!(j.summary(2).tail.len(), 2);
    assert!(j.summary(0).tail.is_empty());
    let page = j.page("", 0, 10, None);
    assert_eq!(j.summary(0).last_seq, page.lines.last().unwrap().seq);
}

#[test]
fn uptime_counts_from_new() {
    let j = Journal::new(1);
    std::thread::sleep(std::time::Duration::from_millis(50));
    let first = j.uptime_secs();
    assert!(
        first >= 0.05,
        "the clock must run from new, not lazily: {first}"
    );
    assert!(j.uptime_secs() >= first);
}

#[test]
fn free_functions_share_one_journal() {
    let _g = lock_tests();
    let before = log_summary(0).last_seq;
    say("t03", "x");

    let page = log_page(before, DEFAULT_RING_CAPACITY);
    let mine: Vec<_> = page.lines.iter().filter(|l| l.tag == "t03").collect();
    assert_eq!(mine.len(), 1);
    assert_eq!(page.boot_id, "");
    assert_eq!(
        log_page_for("abc", before, DEFAULT_RING_CAPACITY, None).boot_id,
        "abc"
    );
}
