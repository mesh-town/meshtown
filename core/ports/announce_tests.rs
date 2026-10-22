//! Tests of the pure part of the announcer: `diff_published` (what to write, what to remove).

use super::*;
use crate::ports::types::Audience;

fn p(port: u16, name: &str) -> Published {
    Published {
        port,
        name: name.into(),
        audience: Audience::All,
    }
}

fn announced(list: &[Published]) -> HashMap<u16, PortRecord> {
    list.iter().map(|p| (p.port, PortRecord::from(p))).collect()
}

#[test]
fn new_port_is_written() {
    let (w, r) = diff_published(&[p(80, "")], &HashMap::new());
    assert_eq!(w, vec![p(80, "")]);
    assert!(r.is_empty());
}

#[test]
fn same_record_is_skipped() {
    let (w, r) = diff_published(&[p(80, "a")], &announced(&[p(80, "a")]));
    assert!(w.is_empty() && r.is_empty());
}

#[test]
fn renamed_port_is_rewritten() {
    let (w, r) = diff_published(&[p(80, "b")], &announced(&[p(80, "a")]));
    assert_eq!(w, vec![p(80, "b")]);
    assert!(r.is_empty());
}

#[test]
fn dropped_port_is_removed() {
    let (w, r) = diff_published(&[], &announced(&[p(80, "")]));
    assert!(w.is_empty());
    assert_eq!(r, vec![80]);
}
