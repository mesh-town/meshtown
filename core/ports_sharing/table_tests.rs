//! Tests of the table of our own publications (`table.rs`).

use std::time::Duration;

use tokio::time::timeout;

use super::*;

fn p(port: u16, name: &str) -> Published {
    Published {
        port,
        name: name.into(),
        audience: Audience::All,
    }
}

#[test]
fn lookup_published() {
    let t = PublishedPortsTable::new(vec![p(80, "")]);
    assert_eq!(t.audience_of(80), Some(Audience::All));
    assert_eq!(t.audience_of(81), None);
}

#[tokio::test]
async fn set_published_wakes_waiter() {
    let t = PublishedPortsTable::new(vec![]);
    let n = t.changed_signal();
    let waiter = crate::node::output::spawn(async move { n.notified().await });
    tokio::task::yield_now().await;
    t.set_published(vec![p(80, "")]);
    timeout(Duration::from_secs(1), waiter)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn set_published_keeps_permit() {
    let t = PublishedPortsTable::new(vec![]);
    t.set_published(vec![p(80, "")]);
    let n = t.changed_signal();
    timeout(Duration::from_millis(100), n.notified())
        .await
        .unwrap();
}

#[test]
fn status_kept_for_same_record() {
    let t = PublishedPortsTable::new(vec![p(80, "a")]);
    t.set_local_state(80, LocalState::Ok);
    t.set_published(vec![p(80, "a")]);
    assert_eq!(t.published_with_state()[0].1, LocalState::Ok);
}

#[test]
fn status_reset_on_changed_record() {
    let t = PublishedPortsTable::new(vec![p(80, "a")]);
    t.set_local_state(80, LocalState::Ok);
    t.set_published(vec![p(80, "b")]);
    assert_eq!(t.published_with_state()[0].1, LocalState::Unknown);
    t.set_local_state(80, LocalState::Ok);
    let mut q = p(80, "b");
    q.audience = Audience::Nodes(vec![]);
    t.set_published(vec![q]);
    assert_eq!(t.published_with_state()[0].1, LocalState::Unknown);
}

#[test]
fn removed_port_forgotten() {
    let t = PublishedPortsTable::new(vec![p(80, "")]);
    t.set_published(vec![]);
    assert_eq!(t.audience_of(80), None);
    t.set_local_state(80, LocalState::Ok);
    assert!(t.published_with_state().is_empty());
}

#[test]
fn list_keeps_order_first_duplicate_wins() {
    let t = PublishedPortsTable::new(vec![p(8080, "a"), p(80, ""), p(8080, "b")]);
    let list = t.published_with_state();
    let ports: Vec<u16> = list.iter().map(|(x, _)| x.port).collect();
    assert_eq!(ports, [8080, 80]);
    assert_eq!(list[0].0.name, "a");
}

#[tokio::test]
async fn mark_local_does_not_wake() {
    let t = PublishedPortsTable::new(vec![p(80, "")]);
    t.set_local_state(80, LocalState::Ok);
    let n = t.changed_signal();
    assert!(timeout(Duration::from_millis(100), n.notified())
        .await
        .is_err());
}
