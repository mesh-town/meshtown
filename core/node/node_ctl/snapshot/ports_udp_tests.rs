//! The protocol axis of `build_ports`: UDP rows, TCP and UDP of one number, `last_error` of an opened port.
//! The helpers `id`, `run`, `view`, `A`, `ME` come from `super` (`ports_tests.rs`).

use super::*;

use crate::config::config_file::NetSlotId;

mod own_publication {
    use super::*;

    #[test]
    fn own_udp_port_has_proto_and_unknown_local() {
        // given: a UDP publication; a UDP port cannot be probed locally
        let own = vec![(published(Proto::Udp, 53), LocalState::Unknown)];

        // when
        let info = when_built(own);

        // then
        assert_eq!(info.published.len(), 1);
        assert_eq!(info.published[0].proto, Proto::Udp);
        assert_eq!(info.published[0].local, "unknown");
    }

    #[test]
    fn udp_local_stays_unknown_whatever_the_table_says() {
        // given: the table claims the number answers
        let own = vec![(published(Proto::Udp, 53), LocalState::Ok)];

        // when
        let info = when_built(own);

        // then
        assert_eq!(info.published[0].local, "unknown");
    }

    #[test]
    fn tcp_port_keeps_its_probe_result_and_proto() {
        // given
        let own = vec![(published(Proto::Tcp, 80), LocalState::Refused)];

        // when
        let info = when_built(own);

        // then
        assert_eq!(info.published[0].proto, Proto::Tcp);
        assert_eq!(info.published[0].local, "refused");
    }

    #[test]
    fn same_number_on_two_protocols_gives_two_rows() {
        // given
        let own = vec![
            (published(Proto::Tcp, 53), LocalState::Ok),
            (published(Proto::Udp, 53), LocalState::Unknown),
        ];

        // when
        let info = when_built(own);

        // then
        let rows: Vec<_> = info.published.iter().map(|p| (p.proto, p.port)).collect();
        assert_eq!(rows, [(Proto::Tcp, 53), (Proto::Udp, 53)]);
    }
}

mod network_rows {
    use super::*;

    #[test]
    fn udp_row_has_proto_and_no_web_url() {
        // given: a UDP port of an online member, on a number a TCP row would get a link for
        let views = [view_over(Proto::Udp, A, 8080)];
        let mut rows = run(&views, &[A], vec![]).network;

        // when
        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        // then
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].proto, rows[0].published), (Proto::Udp, true));
        assert_eq!(rows[0].web_url, None);
    }

    #[test]
    fn tcp_row_of_the_same_number_keeps_its_link() {
        // given: TCP 8080 and UDP 8080 of one member
        let views = [
            view_over(Proto::Tcp, A, 8080),
            view_over(Proto::Udp, A, 8080),
        ];
        let mut rows = run(&views, &[A], vec![]).network;

        // when
        overlay_web_urls(&mut rows, &[address(A, "router.meshtown")]);

        // then
        let url_of = |proto| {
            rows.iter()
                .find(|r| r.proto == proto)
                .and_then(|r| r.web_url.clone())
        };
        assert_eq!(
            url_of(Proto::Tcp).as_deref(),
            Some("http://router.meshtown:8080")
        );
        assert_eq!(url_of(Proto::Udp), None);
    }

    #[test]
    fn two_protocols_give_two_rows() {
        // given: TCP 53 and UDP 53 of one member
        let views = [view_over(Proto::Tcp, A, 53), view_over(Proto::Udp, A, 53)];

        // when
        let info = run(&views, &[A], vec![]);

        // then
        assert_eq!(protos_of(&info), [Proto::Tcp, Proto::Udp]);
    }

    #[test]
    fn opened_ports_of_two_protocols_without_records_give_two_rows() {
        // given: TCP 53 and UDP 53 of one member opened here, nothing published
        let op = vec![
            (
                opened_over(Proto::Tcp, 53, 9053),
                OpenState::Listening,
                None,
            ),
            (
                opened_over(Proto::Udp, 53, 9053),
                OpenState::Listening,
                None,
            ),
        ];

        // when
        let info = run(&[], &[A], op);

        // then
        assert_eq!(protos_of(&info), [Proto::Tcp, Proto::Udp]);
        assert!(info
            .network
            .iter()
            .all(|r| !r.published && r.opened.is_some()));
    }

    #[test]
    fn opened_state_goes_to_the_row_of_its_own_protocol() {
        // given: both protocols are published, only UDP 53 is opened here
        let views = [view_over(Proto::Tcp, A, 53), view_over(Proto::Udp, A, 53)];
        let op = vec![(
            opened_over(Proto::Udp, 53, 2053),
            OpenState::Listening,
            None,
        )];

        // when
        let info = run(&views, &[A], op);

        // then
        assert_eq!(info.network.len(), 2);
        assert_eq!(opened_local_of(&info, Proto::Udp), Some(2053));
        assert_eq!(opened_local_of(&info, Proto::Tcp), None);
    }

    #[test]
    fn offline_author_keeps_the_opened_udp_row() {
        // given: the author is offline, UDP 53 is opened here
        let views = [view_over(Proto::Udp, A, 53)];
        let op = vec![(
            opened_over(Proto::Udp, 53, 2053),
            OpenState::Listening,
            None,
        )];

        // when
        let info = run(&views, &[], op);

        // then
        assert_eq!(protos_of(&info), [Proto::Udp]);
        assert!(!info.network[0].online);
    }
}

mod last_error {
    use super::*;

    #[test]
    fn udp_row_carries_the_reach_of_the_last_failure() {
        // given: the publisher is an older node
        let views = [view_over(Proto::Udp, A, 53)];
        let op = vec![(
            opened_over(Proto::Udp, 53, 2053),
            OpenState::Listening,
            Some(PortReach::Unsupported),
        )];

        // when
        let info = run(&views, &[A], op);

        // then
        let o = info.network[0].opened.as_ref().expect("opened");
        assert_eq!(o.last_error, Some(PortReach::Unsupported));
        assert_eq!(o.state, "listening");
    }

    #[test]
    fn tcp_row_carries_it_too() {
        // given
        let op = vec![(
            opened_over(Proto::Tcp, 80, 9000),
            OpenState::Listening,
            Some(PortReach::Offline),
        )];

        // when
        let info = run(&[], &[], op);

        // then
        let o = info.network[0].opened.as_ref().expect("opened");
        assert_eq!(o.last_error, Some(PortReach::Offline));
    }

    #[test]
    fn is_empty_after_a_success() {
        // given: no failure is remembered
        let op = vec![(
            opened_over(Proto::Udp, 53, 2053),
            OpenState::Listening,
            None,
        )];

        // when
        let info = run(&[view_over(Proto::Udp, A, 53)], &[A], op);

        // then
        assert_eq!(
            info.network[0].opened.as_ref().expect("opened").last_error,
            None
        );
    }

    #[test]
    fn is_kept_next_to_a_bind_failure() {
        // given
        let op = vec![(
            opened_over(Proto::Udp, 53, 2053),
            OpenState::BindFailed("busy".into()),
            Some(PortReach::TimedOut),
        )];

        // when
        let info = run(&[view_over(Proto::Udp, A, 53)], &[A], op);

        // then
        let o = info.network[0].opened.as_ref().expect("opened");
        assert_eq!(
            (o.state.as_str(), o.error.as_deref()),
            ("bind_failed", Some("busy"))
        );
        assert_eq!(o.last_error, Some(PortReach::TimedOut));
    }
}

fn published(proto: Proto, port: u16) -> Published {
    Published {
        proto,
        port,
        name: "x".into(),
        audience: Audience::All,
    }
}

fn when_built(own: Vec<(Published, LocalState)>) -> PortsInfo {
    build_ports(&[], id(ME), &HashMap::new(), &HashSet::new(), own, vec![])
}

/// A record of `proto` that member `author` shares with everyone.
fn view_over(proto: Proto, author: u8, port: u16) -> PortView {
    PortView {
        proto,
        ..view(id(author), port, Some(Audience::All))
    }
}

fn opened_over(proto: Proto, port: u16, local: u16) -> Opened {
    Opened {
        proto,
        ..opened(id(A), port, local)
    }
}

/// Protocols of the network rows, TCP first (`build_ports` does not promise an order).
fn protos_of(info: &PortsInfo) -> Vec<Proto> {
    let mut protos: Vec<_> = info.network.iter().map(|r| r.proto).collect();
    protos.sort();
    protos
}

fn opened_local_of(info: &PortsInfo, proto: Proto) -> Option<u16> {
    let row = info.network.iter().find(|r| r.proto == proto)?;
    row.opened.as_ref().map(|o| o.local)
}

fn address(tag: u8, host: &str) -> MeshAddress {
    MeshAddress {
        net: NetSlotId::parse("ab2c3d").unwrap(),
        peer: id(tag),
        host: host.to_string(),
    }
}
