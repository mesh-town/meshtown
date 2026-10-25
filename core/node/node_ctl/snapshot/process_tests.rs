use super::*;

mod phase_decides {
    use super::*;

    #[test]
    fn no_network_is_offline_with_or_without_cache() {
        assert_eq!(
            phase_status(&Phase::NoNetwork, false),
            Some(NodeStatus::Offline)
        );
        assert_eq!(
            phase_status(&Phase::NoNetwork, true),
            Some(NodeStatus::Offline)
        );
    }

    #[test]
    fn starting_and_stopping_ignore_cache() {
        for has_cache in [false, true] {
            assert_eq!(
                phase_status(&Phase::Starting, has_cache),
                Some(NodeStatus::Starting),
                "Starting with has_cache={has_cache}"
            );
            assert_eq!(
                phase_status(&Phase::Stopping, has_cache),
                Some(NodeStatus::Stopping),
                "Stopping with has_cache={has_cache}"
            );
        }
    }

    #[test]
    fn up_without_cache_is_starting() {
        assert_eq!(phase_status(&Phase::Up, false), Some(NodeStatus::Starting));
    }

    #[test]
    fn up_with_cache_is_left_to_the_link() {
        assert_eq!(phase_status(&Phase::Up, true), None);
    }
}

mod link_decides {
    use super::*;

    #[test]
    fn link_without_members_or_relay_is_offline() {
        assert_eq!(link_status(&snap(0, false)), NodeStatus::Offline);
    }

    #[test]
    fn link_with_a_member_is_online() {
        assert_eq!(link_status(&snap(1, false)), NodeStatus::Online);
    }

    #[test]
    fn link_with_relay_only_is_online() {
        assert_eq!(link_status(&snap(0, true)), NodeStatus::Online);
    }

    #[test]
    fn link_with_member_and_relay_is_online() {
        assert_eq!(link_status(&snap(1, true)), NodeStatus::Online);
    }
}

fn snap(members: u64, relay: bool) -> NodeSnapshot {
    let mut snap = NodeSnapshot::default();
    snap.members.count = members as _;
    snap.transport.relay.connected = relay;
    snap
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
