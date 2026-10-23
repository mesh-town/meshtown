//! `web_url` of the shared ports: the link at the member's mesh address, set from the process proxy's addresses.

use super::*;

use crate::config::config_file::NetSlotId;

mod build_ports {
    use super::*;

    #[test]
    fn leaves_the_link_empty_because_it_has_no_proxy() {
        // given: a shared port of an online member
        let views = [view(id(A), 80, Some(Audience::All))];

        // when
        let info = run(&views, &[A], vec![]);

        // then
        assert_eq!(info.network[0].web_url, None);
    }
}

mod overlay {
    use super::*;

    #[test]
    fn shared_port_gets_the_link_at_the_members_address() {
        // given
        let mut rows = shared_rows(&[(A, 8080)]);

        // when
        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        // then
        assert_eq!(url_of(&rows[0]), Some("http://router.meshtown:8080"));
    }

    #[test]
    fn port_80_has_no_port_in_the_link() {
        let mut rows = shared_rows(&[(A, 80)]);

        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        assert_eq!(url_of(&rows[0]), Some("http://router.meshtown"));
    }

    #[test]
    fn port_443_has_https_and_no_port_in_the_link() {
        let mut rows = shared_rows(&[(A, 443)]);

        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        assert_eq!(url_of(&rows[0]), Some("https://router.meshtown"));
    }

    #[test]
    fn address_with_a_tail_is_used_whole() {
        let mut rows = shared_rows(&[(A, 80)]);

        overlay_web_urls(&mut rows, &[address(A, "router-1a2b.meshtown")]);

        assert_eq!(url_of(&rows[0]), Some("http://router-1a2b.meshtown"));
    }

    #[test]
    fn every_port_of_the_member_gets_its_own_link() {
        let mut rows = shared_rows(&[(A, 80), (A, 22)]);

        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        let urls: Vec<_> = rows.iter().map(url_of).collect();
        assert_eq!(
            urls,
            [Some("http://router.meshtown"), Some("http://router.meshtown:22")]
        );
    }

    #[test]
    fn each_member_gets_the_address_of_its_own_device() {
        let mut rows = shared_rows(&[(A, 80), (B, 80)]);
        let addresses = [address(A, "router.meshtown"), address(B, "printer.meshtown")];

        overlay_web_urls(&mut rows, &addresses);

        let urls: Vec<_> = rows.iter().map(url_of).collect();
        assert_eq!(
            urls,
            [Some("http://router.meshtown"), Some("http://printer.meshtown")]
        );
    }

    #[test]
    fn unpublished_port_opened_here_gets_no_link() {
        // given: a port opened here, but the author has no record for this node
        let op = vec![(opened(id(A), 80, 9000), OpenState::Listening)];
        let mut rows = run(&[], &[A], op).network;
        assert!(!rows[0].published, "the fixture must be an unpublished row");

        // when
        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        // then
        assert_eq!(rows[0].web_url, None);
    }

    #[test]
    fn member_without_an_address_gets_no_link() {
        let mut rows = shared_rows(&[(A, 80)]);

        overlay_web_urls(&mut rows, &[address(B, "printer.meshtown")]);

        assert_eq!(rows[0].web_url, None);
    }

    #[test]
    fn no_addresses_at_all_leave_every_link_empty() {
        let mut rows = shared_rows(&[(A, 80), (B, 443)]);

        overlay_web_urls(&mut rows, &[]);

        assert!(rows.iter().all(|r| r.web_url.is_none()));
    }

    #[test]
    fn clears_a_link_of_a_port_that_is_no_longer_shared() {
        // given: a stale link on an unpublished row
        let op = vec![(opened(id(A), 80, 9000), OpenState::Listening)];
        let mut rows = run(&[], &[A], op).network;
        rows[0].web_url = Some("http://stale.meshtown".into());

        // when
        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        // then
        assert_eq!(rows[0].web_url, None);
    }

    #[test]
    fn address_is_found_by_member_whatever_network_it_is_listed_in() {
        let mut rows = shared_rows(&[(A, 80)]);
        let mut other_net = address(A, "router.meshtown");
        other_net.net = NetSlotId::parse("xy4z56").unwrap();

        overlay_web_urls(&mut rows, &[other_net]);

        assert_eq!(url_of(&rows[0]), Some("http://router.meshtown"));
    }
}

/// Rows of ports that members `(tag, port)` share with everyone; all of them are online.
fn shared_rows(ports: &[(u8, u16)]) -> Vec<NetPort> {
    let views: Vec<_> = ports
        .iter()
        .map(|(tag, port)| view(id(*tag), *port, Some(Audience::All)))
        .collect();
    let tags: Vec<u8> = ports.iter().map(|(tag, _)| *tag).collect();
    let rows = run(&views, &tags, vec![]).network;
    assert_eq!(rows.len(), ports.len(), "every port must be listed");
    order_like(rows, ports)
}

/// `build_ports` does not promise an order; the tests read rows in the order of `ports`.
fn order_like(rows: Vec<NetPort>, ports: &[(u8, u16)]) -> Vec<NetPort> {
    ports
        .iter()
        .map(|(tag, port)| {
            let node = id(*tag).to_string();
            let found = rows.iter().find(|r| r.node == node && r.port == *port);
            found.expect("a row for the port").clone()
        })
        .collect()
}

fn address(tag: u8, host: &str) -> MeshAddress {
    MeshAddress {
        net: NetSlotId::parse("ab2c3d").unwrap(),
        peer: id(tag),
        host: host.to_string(),
    }
}

fn url_of(row: &NetPort) -> Option<&str> {
    row.web_url.as_deref()
}

const B: u8 = 3;
