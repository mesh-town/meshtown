use super::*;

fn cache(members: u64, relay: bool) -> Collected {
    let mut snap = NodeSnapshot::default();
    snap.members.count = members as _;
    snap.transport.relay.connected = relay;
    Collected {
        snap,
        domains: Vec::new(),
        partial: false,
    }
}

#[test]
fn no_network_is_offline_with_or_without_cache() {
    assert_eq!(status_of(&Phase::NoNetwork, None), NodeStatus::Offline);
    let c = cache(3, true);
    assert_eq!(status_of(&Phase::NoNetwork, Some(&c)), NodeStatus::Offline);
}

#[test]
fn starting_and_stopping_ignore_cache() {
    let c = cache(3, true);
    assert_eq!(status_of(&Phase::Starting, None), NodeStatus::Starting);
    assert_eq!(status_of(&Phase::Starting, Some(&c)), NodeStatus::Starting);
    assert_eq!(status_of(&Phase::Stopping, None), NodeStatus::Stopping);
    assert_eq!(status_of(&Phase::Stopping, Some(&c)), NodeStatus::Stopping);
}

#[test]
fn up_without_cache_is_starting() {
    assert_eq!(status_of(&Phase::Up, None), NodeStatus::Starting);
}

#[test]
fn up_with_empty_cache_is_offline() {
    let c = cache(0, false);
    assert_eq!(status_of(&Phase::Up, Some(&c)), NodeStatus::Offline);
}

#[test]
fn up_with_members_is_online() {
    let c = cache(2, false);
    assert_eq!(status_of(&Phase::Up, Some(&c)), NodeStatus::Online);
}

#[test]
fn up_with_relay_only_is_online() {
    let c = cache(0, true);
    assert_eq!(status_of(&Phase::Up, Some(&c)), NodeStatus::Online);
}

#[test]
fn overlay_sets_process_fields() {
    let mut snap = NodeSnapshot::default();
    let log = LogSummary {
        ring_lines: 7,
        ..LogSummary::default()
    };
    let facts = ProcessFacts {
        uptime_secs: 12.5,
        rss_kib: 100,
        rss_peak_kib: 200,
        log,
    };
    overlay_process(&mut snap, facts);
    assert_eq!(snap.now, 12.5);
    assert_eq!(snap.node.rss_kib, 100);
    assert_eq!(snap.node.rss_peak_kib, 200);
    assert_eq!(snap.log.ring_lines, 7);
}

#[test]
fn overlay_keeps_network_fields() {
    let mut snap = NodeSnapshot {
        boot_id: "abcd1234".to_string(),
        ..NodeSnapshot::default()
    };
    snap.node.status = NodeStatus::Online;
    snap.node.id = "x".to_string();
    snap.node.uptime_sec = 42;
    snap.members.count = 3;
    let facts = ProcessFacts {
        uptime_secs: 1.0,
        rss_kib: 1,
        rss_peak_kib: 2,
        log: LogSummary::default(),
    };
    overlay_process(&mut snap, facts);
    assert_eq!(snap.boot_id, "abcd1234");
    assert_eq!(snap.node.status, NodeStatus::Online);
    assert_eq!(snap.node.id, "x");
    assert_eq!(snap.node.uptime_sec, 42);
    assert_eq!(snap.members.count, 3);
}
