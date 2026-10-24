//! Tests for peer rows by name, golden ids, check authors and the SOCKS LAN lists. The helpers come from `super::tests`.

use super::tests::{base, pair, record_at, view};
use super::*;
use crate::transport::net::NodeSecret;

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
