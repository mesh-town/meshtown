use super::*;
use std::time::Duration;

use crate::transport::net::{NodeSecret, PeerId};

/// A network member made from one secret key: its id, which is also the author of document records.
pub(super) fn pair(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

pub(super) fn record_at(verdict: CheckVerdict, at: u64) -> CheckRecord {
    CheckRecord {
        v: crate::site_checks::reachability::CHECK_RECORD_VERSION,
        verdict,
        dns_ms: 1,
        tcp_ms: 2,
        tls_ms: 3,
        http_ms: 4,
        status: 200,
        bytes: 100,
        stalled: false,
        at,
        reason: "ok".to_string(),
    }
}

pub(super) fn view(
    domain: &str,
    author: PeerId,
    at: u64,
    record: Option<CheckRecord>,
) -> CheckView {
    CheckView {
        domain: domain.to_string(),
        author,
        entry_ts_micros: at,
        record,
    }
}

pub(super) fn base(id: PeerId) -> Inputs<'static> {
    Inputs {
        id,
        me: id,
        self_name: None,
        mode: "direct_and_relay",
        members: HashSet::new(),
        peer_names: HashMap::new(),
        neighbors: 0,
        votes: Default::default(),
        socks: None,
        socks_access: (
            crate::config::config_file::ListenAccess::Localhost,
            Vec::new(),
        ),
        socks_ifaces: Default::default(),
        exit_on: false,
        exit_clients: Vec::new(),
        exit_rates: HashMap::new(),
        exits: HashMap::new(),
        countries: HashMap::new(),
        program_versions: HashMap::new(),
        chosen_exits: Vec::new(),
        domain_names: Vec::new(),
        check_views: Vec::new(),
        in_progress: HashSet::new(),
        telemetry: TelemetryView::default(),
        addr: PeerAddr::new(id),
        now_unix: 1_000,
        now_instant: Instant::now(),
        cached_seen: HashMap::new(),
        doc_seen: HashMap::new(),
    }
}

// 1. verdict_kind — all 6 pairs.
#[test]
fn verdict_kind_covers_all_six_pairs() {
    assert_eq!(verdict_kind(CheckVerdict::Ok), VerdictKind::Ok);
    assert_eq!(
        verdict_kind(CheckVerdict::DnsMismatch),
        VerdictKind::DnsMismatch
    );
    assert_eq!(
        verdict_kind(CheckVerdict::NetworkBlocked),
        VerdictKind::NetworkBlocked
    );
    assert_eq!(
        verdict_kind(CheckVerdict::SiteGeoblock),
        VerdictKind::SiteGeoblock
    );
    assert_eq!(verdict_kind(CheckVerdict::Antibot), VerdictKind::Antibot);
    assert_eq!(verdict_kind(CheckVerdict::Down), VerdictKind::Down);
}

// 3. check_info: widening u16/u32 fields → u64; at in the future → age_sec == 0.
#[test]
fn check_info_extends_fields_and_saturates_future_at() {
    let r = record_at(CheckVerdict::Ok, 2_000);
    let info = check_info(&r, 1_000);
    assert_eq!(info.age_sec, 0, "at is in the future relative to now_unix");
    assert_eq!(info.dns_ms, 1);
    assert_eq!(info.tcp_ms, 2);
    assert_eq!(info.tls_ms, 3);
    assert_eq!(info.http_ms, 4);
    assert_eq!(info.status, 200);
    assert_eq!(info.bytes, 100);
    assert_eq!(info.verdict, VerdictKind::Ok);
}

fn offline_live(
    path: Option<PathKindView>,
    rtt_ms: Option<u64>,
    last_seen: Instant,
) -> crate::node::telemetry::PeerLive {
    crate::node::telemetry::PeerLive {
        online: false,
        path,
        rtt_ms,
        last_seen,
    }
}

// 4. build: own row first, is_self, name, exit = Some(flag); other members ascending by
//    short id; offline from tel.peers → online:false, path:None, last_seen_sec by Instant;
//    other members: exit unknown (None).
#[test]
fn build_orders_self_first_and_others_offline_from_telemetry() {
    let my_id = pair(1);
    let other_a = pair(2);
    let other_b = pair(3);
    let mut inputs = base(my_id);
    inputs.self_name = Some("my-node");
    inputs.exit_on = true;
    let seen_at = Instant::now() - Duration::from_secs(42);
    let (live_a, live_b) = (
        offline_live(None, None, seen_at),
        offline_live(
            Some(PathKindView::Relay),
            Some(50),
            Instant::now() - Duration::from_secs(5),
        ),
    );
    inputs.telemetry.peers.insert(other_a, live_a);
    inputs.telemetry.peers.insert(other_b, live_b);

    let collected = build(inputs);
    let peers = collected.snap.peers;
    assert_eq!(peers.len(), 3);
    assert!(peers[0].is_self);
    assert_eq!(peers[0].name.as_deref(), Some("my-node"));
    assert_eq!(peers[0].exit, Some(true));

    let mut expected_order = [
        output::short_peer_id(&other_a),
        output::short_peer_id(&other_b),
    ];
    expected_order.sort();
    let actual_order: Vec<&str> = peers[1..].iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        actual_order,
        expected_order
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );

    for p in &peers[1..] {
        assert!(!p.online, "not network members — offline");
        assert_eq!(p.exit, None, "another member's exit is unknown");
        assert_eq!(p.path, None, "path only when online");
        assert!(
            p.last_seen_sec > 0,
            "last visit comes from PeerLive.last_seen"
        );
    }
}

// 5. build: checks keys = peers[].id; no record → None; record: None → None; own record
//    under own short id.
#[test]
fn build_domain_checks_keyed_by_peer_short_id() {
    let my_id = pair(1);
    let friend_id = pair(2);
    let mut inputs = base(my_id);
    inputs.members = [friend_id].into_iter().collect();
    inputs.domain_names = vec!["example.com".to_string(), "other.com".to_string()];
    inputs.check_views = vec![
        view(
            "example.com",
            my_id,
            10,
            Some(record_at(CheckVerdict::Ok, 10)),
        ),
        view("example.com", friend_id, 10, None),
    ];

    let collected = build(inputs);
    let example = collected
        .snap
        .domains
        .iter()
        .find(|d| d.name == "example.com")
        .unwrap();
    let my_key = output::short_peer_id(&my_id);
    let friend_key = output::short_peer_id(&friend_id);
    assert!(matches!(example.checks.get(&my_key), Some(Some(_))));
    assert!(matches!(example.checks.get(&friend_key), Some(None)));

    let other = collected
        .snap
        .domains
        .iter()
        .find(|d| d.name == "other.com")
        .unwrap();
    assert!(matches!(other.checks.get(&my_key), Some(None)));
    assert!(matches!(other.checks.get(&friend_key), Some(None)));
}

// 6. checked/stale: fresh record → stale:false; record older than 7200 s → true; an author present only
//    in records shows up in peers with online:false.
#[test]
fn checked_and_stale_thresholds_and_record_only_author() {
    let my_id = pair(1);
    let stranger_id = pair(9);
    let mut inputs = base(my_id);
    inputs.now_unix = 10_000;
    inputs.domain_names = vec!["a.com".to_string(), "b.com".to_string()];
    inputs.check_views = vec![
        view("a.com", my_id, 1, Some(record_at(CheckVerdict::Ok, 9_990))),
        view(
            "a.com",
            stranger_id,
            1,
            Some(record_at(CheckVerdict::Ok, 10_000 - 7_201)),
        ),
        view(
            "b.com",
            stranger_id,
            1,
            Some(record_at(CheckVerdict::Ok, 10_000 - 7_201)),
        ),
    ];

    let collected = build(inputs);
    let me_peer = collected.snap.peers.iter().find(|p| p.is_self).unwrap();
    assert!(!me_peer.stale, "fresh record — not stale");
    assert_eq!(me_peer.checked, 1);

    let stranger_peer = collected
        .snap
        .peers
        .iter()
        .find(|p| p.id == output::short_peer_id(&stranger_id))
        .expect("the author appears in peers only through check records");
    assert!(!stranger_peer.online, "not a network member — offline");
    assert!(stranger_peer.stale, "records older than 7200 s");
    assert_eq!(stranger_peer.checked, 2);
}

// 7. Name of another member: a peers[] row takes the name from Inputs.peer_names.
#[test]
fn other_peer_takes_name_from_members() {
    let my_id = pair(1);
    let x = pair(2);
    let mut inputs = base(my_id);
    inputs.members = [x].into_iter().collect();
    inputs.peer_names = [(x, "Kitchen".to_string())].into_iter().collect();

    let peers = build_peers(&inputs);
    let x_peer = peers
        .iter()
        .find(|p| p.id == output::short_peer_id(&x))
        .expect("x is in members, the row exists");
    assert_eq!(x_peer.name, Some("Kitchen".to_string()));
}

#[test]
fn other_peer_without_name_is_none() {
    let my_id = pair(1);
    let x = pair(2);
    let mut inputs = base(my_id);
    inputs.members = [x].into_iter().collect();

    let peers = build_peers(&inputs);
    let x_peer = peers
        .iter()
        .find(|p| p.id == output::short_peer_id(&x))
        .expect("x is in members, the row exists");
    assert_eq!(x_peer.name, None);
}

// 8. Exit by name: route.exits[] takes the name of the same member from peers[].
#[test]
fn chosen_exit_inherits_peer_name() {
    let my_id = pair(1);
    let x = pair(2);
    let mut inputs = base(my_id);
    inputs.members = [x].into_iter().collect();
    inputs.peer_names = [(x, "Kitchen".to_string())].into_iter().collect();
    inputs.chosen_exits = vec![x];

    let collected = build(inputs);
    assert_eq!(
        collected.snap.route.exits[0].name,
        Some("Kitchen".to_string())
    );
}

fn golden_id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

#[test]
fn golden_snapshot_id_is_ten_hex_and_id_full_is_64_hex() {
    let mine = golden_id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let other = golden_id_of("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
    // RFC 8032 §7.1 anchor
    assert_eq!(
        mine.to_string(),
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    );
    assert_eq!(
        other.to_string(),
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
    );
    let mut inputs = base(mine);
    inputs.members = [other].into_iter().collect();

    let collected = build(inputs);

    let peers = &collected.snap.peers;
    assert_eq!(peers.len(), 2);
    assert!(peers[0].is_self);
    assert_eq!(peers[0].id, "d75a980182");
    assert_eq!(
        peers[0].id_full,
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    );
    assert_eq!(peers[1].id, "3d4017c3e8");
    assert_eq!(
        peers[1].id_full,
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
    );
}

/// Authors of check records (`Inputs.check_views`) are ready-made `PeerId`s: `peers[]` takes them as is,
/// while domains outside the list create no rows and do not grow the `checked` counter.
mod check_authors {
    use super::*;

    #[test]
    fn author_of_unlisted_domain_has_no_peer_row() {
        // given: a record from another node for a domain that is not in this node's list.
        let stranger = pair(9);
        let mut inputs = base(pair(1));
        inputs.domain_names = vec!["listed.com".to_string()];
        inputs.check_views = vec![view(
            "other.com",
            stranger,
            1,
            Some(record_at(CheckVerdict::Ok, 900)),
        )];

        // when
        let peers = build_peers(&inputs);

        // then
        let found = peers.iter().any(|p| p.id_full == stranger.to_string());
        assert!(
            !found,
            "the author of a record for a foreign domain is not a snapshot member"
        );
    }

    #[test]
    fn unlisted_domain_records_do_not_count_as_checked() {
        // given: a network member with two records — for a domain from the list and for a domain outside it.
        let member = pair(2);
        let mut inputs = base(pair(1));
        inputs.members = [member].into_iter().collect();
        inputs.domain_names = vec!["listed.com".to_string()];
        inputs.check_views = vec![
            view(
                "listed.com",
                member,
                1,
                Some(record_at(CheckVerdict::Ok, 900)),
            ),
            view(
                "other.com",
                member,
                1,
                Some(record_at(CheckVerdict::Ok, 950)),
            ),
        ];

        // when
        let peers = build_peers(&inputs);

        // then
        let row = peers
            .iter()
            .find(|p| p.id_full == member.to_string())
            .expect("member is in members, the row exists");
        assert_eq!(row.checked, 1, "only the domain from the list counts");
    }
}

#[test]
fn socks_lan_lists_every_address_with_port_and_iface() {
    let sock: Option<SocketAddr> = Some("127.0.0.1:1080".parse().unwrap());
    let lan = vec![
        (
            "br-lan".to_string(),
            std::net::Ipv4Addr::new(192, 168, 8, 1),
        ),
        ("eth1".to_string(), std::net::Ipv4Addr::new(10, 0, 0, 5)),
    ];
    let got = socks_lan(sock, lan.clone());
    assert_eq!(got.len(), 2);
    assert_eq!(
        (got[0].addr.as_str(), got[0].iface.as_str()),
        ("192.168.8.1:1080", "br-lan")
    );
    assert_eq!(got[1].addr, "10.0.0.5:1080");
    assert!(socks_lan(None, lan).is_empty());
    assert!(socks_lan(sock, Vec::new()).is_empty());
}

#[test]
fn socks_info_listen_stays_loopback_in_both_modes() {
    use crate::config::config_file::ListenAccess::{Localhost, Localnet};
    for (mode, lan) in [
        (Localhost, vec![]),
        (
            Localnet,
            vec![("br-lan".to_string(), std::net::Ipv4Addr::new(10, 0, 0, 2))],
        ),
    ] {
        let mut i = base(pair(1));
        i.socks = Some("127.0.0.1:1080".parse().unwrap());
        i.socks_access = (mode, lan);
        let snap = build(i).snap;
        assert_eq!(snap.socks.listen.as_deref(), Some("127.0.0.1:1080"));
        assert_eq!(snap.socks.access, mode.as_str());
        assert_eq!(!snap.socks.lan.is_empty(), mode == Localnet);
    }
}

#[test]
fn socks_ifaces_comes_from_the_filter() {
    use crate::config::config_file::IfaceFilter;
    let mut i = base(pair(1));
    i.socks_ifaces = IfaceFilter::Only(vec!["br-lan".to_string()]);
    assert_eq!(build(i).snap.socks.ifaces, Some(vec!["br-lan".to_string()]));
    assert_eq!(build(base(pair(1))).snap.socks.ifaces, None);
}
