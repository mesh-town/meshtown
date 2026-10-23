//! Tests for `ring.rs`: seq sequence, ring overflow, pages after/limit, tags.

use super::*;

#[test]
fn seq_starts_at_one_and_grows() {
    let mut ring = LogRing::new(10);
    ring.push(0.0, "web", "a".to_string());
    ring.push(0.0, "web", "b".to_string());
    ring.push(0.0, "web", "c".to_string());
    let seqs: Vec<u64> = ring
        .page("x", 0, 10, None)
        .lines
        .iter()
        .map(|l| l.seq)
        .collect();
    assert_eq!(seqs, vec![1, 2, 3]);
}

#[test]
fn overflow_drops_oldest() {
    let mut ring = LogRing::new(3);
    for i in 0..5 {
        ring.push(0.0, "web", format!("line{i}"));
    }
    let page = ring.page("x", 0, 10, None);
    let seqs: Vec<u64> = page.lines.iter().map(|l| l.seq).collect();
    assert_eq!(seqs, vec![3, 4, 5]);
    assert_eq!(page.first_seq, 3);
    assert_eq!(page.last_seq, 5);
}

#[test]
fn page_after_and_limit() {
    let mut ring = LogRing::new(20);
    for _ in 0..10 {
        ring.push(0.0, "web", "x".to_string());
    }
    let page = ring.page("x", 4, 3, None);
    let seqs: Vec<u64> = page.lines.iter().map(|l| l.seq).collect();
    assert_eq!(seqs, vec![5, 6, 7]);
    assert_eq!(page.first_seq, 1);
    assert_eq!(page.last_seq, 10);

    let page2 = ring.page("x", 10, 5, None);
    assert!(page2.lines.is_empty());
    assert_eq!(page2.first_seq, 1);
    assert_eq!(page2.last_seq, 10);
}

#[test]
fn empty_bounds_are_zero() {
    let ring = LogRing::new(5);
    let page = ring.page("x", 0, 10, None);
    assert_eq!(page.first_seq, 0);
    assert_eq!(page.last_seq, 0);
    let summary = ring.summary(5);
    assert_eq!(summary.first_seq, 0);
    assert_eq!(summary.last_seq, 0);
}

#[test]
fn shrink_drops_oldest() {
    let mut ring = LogRing::new(20);
    for _ in 0..10 {
        ring.push(0.0, "web", "x".to_string());
    }
    ring.set_capacity(4);
    let page = ring.page("x", 0, 10, None);
    let seqs: Vec<u64> = page.lines.iter().map(|l| l.seq).collect();
    assert_eq!(seqs, vec![7, 8, 9, 10]);
    assert_eq!(ring.summary(10).ring_lines, 4);
}

#[test]
fn clear_keeps_seq_growing() {
    let mut ring = LogRing::new(10);
    ring.push(0.0, "web", "a".to_string());
    ring.push(0.0, "web", "b".to_string());
    ring.clear();
    ring.push(0.0, "web", "c".to_string());
    let page = ring.page("x", 0, 10, None);
    assert_eq!(page.lines.len(), 1);
    assert_eq!(page.lines[0].seq, 3);
    assert_eq!(page.first_seq, 3);
}

#[test]
fn tags_sorted_unique() {
    let mut ring = LogRing::new(10);
    ring.push(0.0, "web", "a".to_string());
    ring.push(0.0, "dht", "b".to_string());
    ring.push(0.0, "web", "c".to_string());
    ring.push(0.0, "check", "d".to_string());
    let summary = ring.summary(6);
    assert_eq!(
        summary.tags,
        vec!["check".to_string(), "dht".to_string(), "web".to_string()]
    );
}

#[test]
fn zero_capacity_is_one() {
    let mut ring = LogRing::new(0);
    ring.push(0.0, "web", "a".to_string());
    ring.push(0.0, "web", "b".to_string());
    let page = ring.page("x", 0, 10, None);
    assert_eq!(page.lines.len(), 1);
    assert_eq!(ring.summary(10).ring_lines, 1);
}
