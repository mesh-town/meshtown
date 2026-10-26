//! Tests of the table of our own publications (`table.rs`).

use std::time::Duration;

use tokio::time::timeout;

use super::*;

fn p(port: u16, name: &str) -> Published {
    pp(Proto::Tcp, port, name)
}

fn pp(proto: Proto, port: u16, name: &str) -> Published {
    Published {
        port,
        proto,
        name: name.into(),
        audience: Audience::All,
    }
}

#[test]
fn lookup_published() {
    let t = PublishedPortsTable::new(vec![p(80, "")]);
    assert_eq!(t.audience_of(Proto::Tcp, 80), Some(Audience::All));
    assert_eq!(t.audience_of(Proto::Tcp, 81), None);
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
    t.set_local_state(Proto::Tcp, 80, LocalState::Ok);
    t.set_published(vec![p(80, "a")]);
    assert_eq!(t.published_with_state()[0].1, LocalState::Ok);
}

#[test]
fn status_reset_on_changed_record() {
    let t = PublishedPortsTable::new(vec![p(80, "a")]);
    t.set_local_state(Proto::Tcp, 80, LocalState::Ok);
    t.set_published(vec![p(80, "b")]);
    assert_eq!(t.published_with_state()[0].1, LocalState::Unknown);
    t.set_local_state(Proto::Tcp, 80, LocalState::Ok);
    let mut q = p(80, "b");
    q.audience = Audience::Nodes(vec![]);
    t.set_published(vec![q]);
    assert_eq!(t.published_with_state()[0].1, LocalState::Unknown);
}

#[test]
fn removed_port_forgotten() {
    let t = PublishedPortsTable::new(vec![p(80, "")]);
    t.set_published(vec![]);
    assert_eq!(t.audience_of(Proto::Tcp, 80), None);
    t.set_local_state(Proto::Tcp, 80, LocalState::Ok);
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
    t.set_local_state(Proto::Tcp, 80, LocalState::Ok);
    let n = t.changed_signal();
    assert!(timeout(Duration::from_millis(100), n.notified())
        .await
        .is_err());
}

mod two_protocols {
    use super::*;

    fn state_of(t: &PublishedPortsTable, proto: Proto, port: u16) -> Option<LocalState> {
        t.published_with_state()
            .into_iter()
            .find(|(p, _)| (p.proto, p.port) == (proto, port))
            .map(|(_, s)| s)
    }

    #[test]
    fn same_number_on_tcp_and_udp_gives_two_rows() {
        // given: TCP 53 and UDP 53 in one list
        let t = PublishedPortsTable::new(vec![p(53, "a"), pp(Proto::Udp, 53, "b")]);
        // then: both rows are kept, in the order of the list
        let rows: Vec<(Proto, u16)> = t
            .published_with_state()
            .iter()
            .map(|(x, _)| (x.proto, x.port))
            .collect();
        assert_eq!(rows, [(Proto::Tcp, 53), (Proto::Udp, 53)]);
    }

    #[test]
    fn repeated_protocol_and_port_keeps_the_first() {
        let t = PublishedPortsTable::new(vec![
            pp(Proto::Udp, 53, "first"),
            p(53, "tcp"),
            pp(Proto::Udp, 53, "second"),
        ]);
        let list = t.published_with_state();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[0].0.proto, list[0].0.name.as_str()),
            (Proto::Udp, "first")
        );
    }

    #[test]
    fn audience_is_looked_up_by_protocol_and_port() {
        // given: UDP 53 for one node, TCP 53 for all
        let mut udp = pp(Proto::Udp, 53, "");
        udp.audience = Audience::Nodes(vec![]);
        let t = PublishedPortsTable::new(vec![p(53, ""), udp]);
        // then: each protocol answers with its own circle
        assert_eq!(t.audience_of(Proto::Tcp, 53), Some(Audience::All));
        assert_eq!(t.audience_of(Proto::Udp, 53), Some(Audience::Nodes(vec![])));
    }

    #[test]
    fn only_tcp_published_means_udp_of_that_number_is_absent() {
        let t = PublishedPortsTable::new(vec![p(53, "")]);
        assert_eq!(t.audience_of(Proto::Udp, 53), None);
    }

    #[test]
    fn only_udp_published_means_tcp_of_that_number_is_absent() {
        let t = PublishedPortsTable::new(vec![pp(Proto::Udp, 53, "")]);
        assert_eq!(t.audience_of(Proto::Tcp, 53), None);
    }

    #[test]
    fn local_state_is_set_for_one_protocol_only() {
        // given: TCP 53 and UDP 53
        let t = PublishedPortsTable::new(vec![p(53, ""), pp(Proto::Udp, 53, "")]);
        // when: the check result of TCP 53 is recorded
        t.set_local_state(Proto::Tcp, 53, LocalState::Refused);
        // then: UDP 53 stays unchecked
        assert_eq!(state_of(&t, Proto::Tcp, 53), Some(LocalState::Refused));
        assert_eq!(state_of(&t, Proto::Udp, 53), Some(LocalState::Unknown));
    }

    #[test]
    fn local_state_of_a_missing_protocol_is_ignored() {
        let t = PublishedPortsTable::new(vec![p(53, "")]);
        t.set_local_state(Proto::Udp, 53, LocalState::Ok);
        assert_eq!(state_of(&t, Proto::Tcp, 53), Some(LocalState::Unknown));
        assert_eq!(t.published_with_state().len(), 1);
    }

    #[test]
    fn status_of_tcp_is_not_carried_to_udp_of_the_same_number() {
        // given: TCP 53 was checked
        let t = PublishedPortsTable::new(vec![p(53, "a")]);
        t.set_local_state(Proto::Tcp, 53, LocalState::Ok);
        // when: the list now holds UDP 53 with the same name instead
        t.set_published(vec![pp(Proto::Udp, 53, "a")]);
        // then: the new row starts as unchecked
        assert_eq!(state_of(&t, Proto::Udp, 53), Some(LocalState::Unknown));
    }

    #[test]
    fn status_of_tcp_survives_when_udp_of_the_same_number_is_added() {
        let t = PublishedPortsTable::new(vec![p(53, "a")]);
        t.set_local_state(Proto::Tcp, 53, LocalState::Ok);
        t.set_published(vec![p(53, "a"), pp(Proto::Udp, 53, "a")]);
        assert_eq!(state_of(&t, Proto::Tcp, 53), Some(LocalState::Ok));
        assert_eq!(state_of(&t, Proto::Udp, 53), Some(LocalState::Unknown));
    }

    #[test]
    fn removing_udp_keeps_tcp_of_the_same_number() {
        let t = PublishedPortsTable::new(vec![p(53, ""), pp(Proto::Udp, 53, "")]);
        t.set_published(vec![p(53, "")]);
        assert_eq!(t.audience_of(Proto::Tcp, 53), Some(Audience::All));
        assert_eq!(t.audience_of(Proto::Udp, 53), None);
    }
}
