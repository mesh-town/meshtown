//! Continuation of `build_tests.rs` (tests 7–11; the file is split at the 400-line ceiling):
//! null invariants, own addresses, own path, `link_status`, the domain fields of `Collected`.
//! The helpers `base`/`pair` come from `super::tests` (`pub(super)`); this module is a sibling of `mod tests`,
//! not its descendant (like `exit_tests`).

use std::time::Instant;

use crate::transport::net::endpoint_shim::from_endpoint_addr;
use crate::transport::net::endpoint_shim::{EndpointAddr, RelayUrl, TransportAddr};
use crate::web_gui::NodeStatus;

use super::tests::{base, pair};
use super::*;

// 7. Null invariants: relay_connected:true without url → connected:false; dht_joined without
//    last_publish → joined:false, with it → Some(_).
#[test]
fn invariants_hold_for_relay_and_dht() {
    let my_id = pair(1);
    let inputs = base(my_id);
    let collected = build(inputs);
    assert!(!collected.snap.transport.relay.connected);
    assert!(collected.snap.transport.relay.url.is_none());

    let mut inputs2 = base(my_id);
    inputs2.telemetry.dht_joined = true;
    inputs2.telemetry.last_publish = None;
    let collected2 = build(inputs2);
    assert!(!collected2.snap.transport.dht.joined);
    assert!(collected2.snap.transport.dht.last_publish_sec.is_none());

    let mut inputs3 = base(my_id);
    inputs3.telemetry.dht_joined = true;
    inputs3.telemetry.last_publish = Some(Instant::now());
    let collected3 = build(inputs3);
    assert!(collected3.snap.transport.dht.joined);
    assert!(collected3.snap.transport.dht.last_publish_sec.is_some());
}

// 8. self_addresses: order v4, v6, relay; the prefix before @ parses back into the same id;
//    the relay tail parses back into the same URL.
#[test]
fn self_addresses_order_and_roundtrip() {
    let my_id = pair(1);
    let v4: SocketAddr = "192.168.1.14:47821".parse().unwrap();
    let v6: SocketAddr = "[2001:db8::1]:47821".parse().unwrap();
    let relay: RelayUrl = "https://relay.example./".parse().unwrap();
    let mut inputs = base(my_id);
    inputs.addr = from_endpoint_addr(&EndpointAddr::from_parts(
        my_id,
        [
            TransportAddr::Ip(v4),
            TransportAddr::Ip(v6),
            TransportAddr::Relay(relay.clone()),
        ],
    ));

    let collected = build(inputs);
    let addrs = &collected.snap.transport.self_addresses;
    assert_eq!(addrs.len(), 3);
    let (id_v4, rest_v4) = addrs[0].split_once('@').unwrap();
    let (id_v6, rest_v6) = addrs[1].split_once('@').unwrap();
    let (id_relay, rest_relay) = addrs[2].split_once('@').unwrap();
    assert_eq!(id_v4.parse::<PeerId>().unwrap(), my_id);
    assert_eq!(id_v6.parse::<PeerId>().unwrap(), my_id);
    assert_eq!(id_relay.parse::<PeerId>().unwrap(), my_id);
    assert_eq!(rest_v4.parse::<SocketAddr>().unwrap(), v4);
    assert_eq!(rest_v6.parse::<SocketAddr>().unwrap(), v6);
    assert_eq!(rest_relay, "https://relay.example./");
    assert_eq!(rest_relay.parse::<RelayUrl>().unwrap(), relay);
}

// 9. self_path: the path to itself follows the paths of the members: a direct one wins, else a relayed one,
//    else nothing.
mod self_path {
    use super::*;

    #[test]
    fn self_path_direct_wins() {
        // given: one member over a relay, another one direct
        let inputs = with_paths(&[(2, PathKindView::Relay), (3, PathKindView::Direct)]);
        // when
        let collected = build(inputs);
        // then
        assert_eq!(
            collected.snap.transport.self_path,
            Some(PathKindView::Direct)
        );
    }

    #[test]
    fn self_path_relay_when_only_relayed_peers() {
        // given: the only live member is reached through a relay
        let inputs = with_paths(&[(2, PathKindView::Relay)]);
        // when
        let collected = build(inputs);
        // then
        assert_eq!(
            collected.snap.transport.self_path,
            Some(PathKindView::Relay)
        );
    }

    #[test]
    fn self_path_null_without_peers() {
        // given: no member has a path
        let inputs = base(pair(1));
        // when
        let collected = build(inputs);
        // then
        assert_eq!(collected.snap.transport.self_path, None);
    }

    #[test]
    fn self_path_null_for_a_member_without_a_path() {
        // given: a member is known but has no established path
        let mut inputs = with_paths(&[]);
        inputs.members = [pair(2)].into_iter().collect();
        // when
        let collected = build(inputs);
        // then
        assert_eq!(collected.snap.transport.self_path, None);
    }
}

/// Inputs of node 1 whose members, by seed, have the given paths.
fn with_paths(paths: &[(u8, PathKindView)]) -> Inputs<'static> {
    let mut inputs = base(pair(1));
    for (seed, path) in paths {
        let id = pair(*seed);
        inputs.members.insert(id);
        inputs.telemetry.peers.insert(
            id,
            crate::node::telemetry::PeerLive {
                online: true,
                path: Some(*path),
                rtt_ms: Some(5),
                last_seen: Instant::now(),
            },
        );
    }
    inputs
}

// 10. link_status (on the snapshot): members > 0 → Online; relay only → Online; nothing →
//     Offline.
#[test]
fn link_status_online_or_offline() {
    let my_id = pair(1);
    let friend_id = pair(2);
    let mut inputs = base(my_id);
    inputs.members = [friend_id].into_iter().collect();
    let collected = build(inputs);
    assert_eq!(link_status(&collected.snap), NodeStatus::Online);

    let inputs3 = base(my_id);
    let collected3 = build(inputs3);
    assert_eq!(link_status(&collected3.snap), NodeStatus::Offline);
}

// 11. Collected.domains == list; checking from in_progress.
#[test]
fn collected_domains_checking_and_blocked() {
    let my_id = pair(1);
    let blocked_domain = "blocked.com".to_string();
    let mut inputs = base(my_id);
    inputs.domain_names = vec!["checking.com".to_string(), blocked_domain.clone()];
    inputs.in_progress = ["checking.com".to_string()].into_iter().collect();

    let collected = build(inputs);
    assert_eq!(
        collected.domains,
        vec!["checking.com".to_string(), blocked_domain.clone()]
    );

    let checking = collected
        .snap
        .domains
        .iter()
        .find(|d| d.name == "checking.com")
        .unwrap();
    assert!(checking.checking);

    let blocked_d = collected
        .snap
        .domains
        .iter()
        .find(|d| d.name == blocked_domain)
        .unwrap();
    assert!(!blocked_d.checking);
}

// 12. read_sec: a read 7 s ago -> Some(7); no read -> null in /api/state.
#[test]
fn read_sec_reflects_last_dht_read() {
    let my_id = pair(1);
    let mut inputs = base(my_id);
    inputs.telemetry.dht_read = Some((inputs.now_instant - std::time::Duration::from_secs(7), 3));
    let record = build(inputs).snap.transport.record;
    assert_eq!(record.read_sec, Some(7));

    let record = build(base(my_id)).snap.transport.record;
    assert_eq!(record.read_sec, None);
    let json = serde_json::to_value(&record).unwrap();
    assert!(json["read_sec"].is_null() && json.as_object().unwrap().contains_key("read_sec"));
}
