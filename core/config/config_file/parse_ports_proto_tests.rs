//! The `proto` key of `ports.published` / `ports.opened` elements: "tcp" (default) or "udp".

use super::*;

mod element_without_proto {
    use super::*;

    #[test]
    fn element_without_proto_reads_as_tcp() {
        // given: both lists, no `proto` key anywhere
        let body = format!(
            r#"{{"published":[{{"port":80,"name":"web","for":"all"}}],
                 "opened":[{{"node":"{}","port":80,"local":9000}}]}}"#,
            id(1)
        );
        // when
        let p = parse(&body);
        // then
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        assert_eq!(p.settings.net.ports_published[0].proto, Proto::Tcp);
        assert_eq!(p.settings.net.ports_opened[0].proto, Proto::Tcp);
    }

    #[test]
    fn explicit_tcp_is_the_same_as_absent() {
        // given
        let body = format!(
            r#"{{"published":[{{"port":80,"proto":"tcp","name":"web","for":"all"}}],
                 "opened":[{{"node":"{}","port":80,"proto":"tcp","local":9000}}]}}"#,
            id(1)
        );
        // when
        let p = parse(&body);
        // then
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        assert_eq!(p.settings.net.ports_published[0].proto, Proto::Tcp);
        assert_eq!(p.settings.net.ports_opened[0].proto, Proto::Tcp);
    }
}

mod element_with_udp {
    use super::*;

    #[test]
    fn udp_element_keeps_its_protocol() {
        // given
        let body = format!(
            r#"{{"published":[{{"port":53,"proto":"udp","name":"dns","for":"all"}}],
                 "opened":[{{"node":"{}","port":53,"proto":"udp","local":2053}}]}}"#,
            id(1)
        );
        // when
        let p = parse(&body);
        // then
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        let published = &p.settings.net.ports_published[0];
        assert_eq!((published.proto, published.port), (Proto::Udp, 53));
        let opened = &p.settings.net.ports_opened[0];
        assert_eq!(
            (opened.proto, opened.port, opened.local),
            (Proto::Udp, 53, 2053)
        );
    }

    #[test]
    fn other_fields_of_a_udp_element_are_read_as_for_tcp() {
        // given
        let body = r#"{"published":[{"port":53,"proto":"udp","name":"dns","for":"all"}]}"#;
        // when
        let p = parse(body);
        // then
        let published = &p.settings.net.ports_published[0];
        assert_eq!(published.name, "dns");
        assert_eq!(published.audience, Audience::All);
    }
}

mod unknown_protocol {
    use super::*;

    const BAD: [&str; 6] = ["\"sctp\"", "5", "null", "\"TCP\"", "\"\"", "[\"udp\"]"];

    #[test]
    fn unknown_protocol_rejects_the_whole_published_list() {
        for bad in BAD {
            // given: a good element next to the bad one
            let body = format!(
                r#"{{"published":[{{"port":80,"name":"a","for":"all"}},
                                  {{"port":81,"proto":{bad},"name":"b","for":"all"}}]}}"#
            );
            // then
            assert_rejected(&body, "ports.published", bad);
        }
    }

    #[test]
    fn unknown_protocol_rejects_the_whole_opened_list() {
        for bad in BAD {
            // given: a good element next to the bad one
            let body = format!(
                r#"{{"opened":[{{"node":"{n}","port":80,"local":9000}},
                               {{"node":"{n}","port":81,"proto":{bad},"local":9001}}]}}"#,
                n = id(1)
            );
            // then
            assert_rejected(&body, "ports.opened", bad);
        }
    }
}

mod same_number_on_two_protocols {
    use super::*;

    #[test]
    fn same_number_on_two_protocols_is_allowed() {
        // given: TCP 53 and UDP 53 in both lists; the opened ones share one local number
        let n = id(1);
        let body = format!(
            r#"{{"published":[{{"port":53,"name":"a","for":"all"}},
                              {{"port":53,"proto":"udp","name":"b","for":"all"}}],
                 "opened":[{{"node":"{n}","port":53,"local":2053}},
                           {{"node":"{n}","port":53,"proto":"udp","local":2053}}]}}"#
        );
        // when
        let p = parse(&body);
        // then
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        let published: Vec<_> = p
            .settings
            .net
            .ports_published
            .iter()
            .map(|x| (x.proto, x.port))
            .collect();
        assert_eq!(published, [(Proto::Tcp, 53), (Proto::Udp, 53)]);
        let opened: Vec<_> = p
            .settings
            .net
            .ports_opened
            .iter()
            .map(|x| (x.proto, x.local))
            .collect();
        assert_eq!(opened, [(Proto::Tcp, 2053), (Proto::Udp, 2053)]);
    }

    #[test]
    fn repeated_protocol_and_port_is_rejected() {
        // given: UDP 53 twice
        let body = r#"{"published":[{"port":53,"proto":"udp","name":"a","for":"all"},
                                    {"port":53,"proto":"udp","name":"b","for":"all"}]}"#;
        // then
        assert_rejected(body, "ports.published", "published udp 53 twice");
    }

    #[test]
    fn repeated_tcp_port_with_and_without_the_proto_key_is_rejected() {
        // given: the same TCP port, once implicit and once explicit
        let body = r#"{"published":[{"port":53,"name":"a","for":"all"},
                                    {"port":53,"proto":"tcp","name":"b","for":"all"}]}"#;
        // then
        assert_rejected(body, "ports.published", "tcp 53 twice");
    }

    #[test]
    fn repeated_node_protocol_and_port_is_rejected() {
        // given: (node, udp, 53) twice on different locals
        let n = id(1);
        let body = format!(
            r#"{{"opened":[{{"node":"{n}","port":53,"proto":"udp","local":2053}},
                           {{"node":"{n}","port":53,"proto":"udp","local":2054}}]}}"#
        );
        // then
        assert_rejected(&body, "ports.opened", "(node, udp, 53) twice");
    }

    #[test]
    fn repeated_protocol_and_local_is_rejected() {
        // given: UDP local 2053 twice for different ports
        let n = id(1);
        let body = format!(
            r#"{{"opened":[{{"node":"{n}","port":53,"proto":"udp","local":2053}},
                           {{"node":"{n}","port":54,"proto":"udp","local":2053}}]}}"#
        );
        // then
        assert_rejected(&body, "ports.opened", "(udp, 2053) twice");
    }

    #[test]
    fn same_port_of_another_node_on_another_protocol_is_allowed() {
        // given: node 1 TCP 53 on local 1, node 2 UDP 53 on local 1
        let body = format!(
            r#"{{"opened":[{{"node":"{}","port":53,"local":2053}},
                           {{"node":"{}","port":53,"proto":"udp","local":2053}}]}}"#,
            id(1),
            id(2)
        );
        // when
        let p = parse(&body);
        // then
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        assert_eq!(p.settings.net.ports_opened.len(), 2);
    }
}

mod list_limit {
    use super::*;

    #[test]
    fn list_limit_counts_both_protocols() {
        for (count, accepted) in [(0, true), (1, true), (64, true), (65, false)] {
            // given: `count` entries, alternating TCP and UDP over the same numbers
            let published = format!(r#"{{"published":{}}}"#, published_mixed(count));
            let opened = format!(r#"{{"opened":{}}}"#, opened_mixed(count));
            // when
            let (pp, po) = (parse(&published), parse(&opened));
            // then
            assert_eq!(
                pp.settings.net.ports_published.len(),
                if accepted { count } else { 0 },
                "published {count}"
            );
            assert_eq!(
                po.settings.net.ports_opened.len(),
                if accepted { count } else { 0 },
                "opened {count}"
            );
            assert_eq!(
                pp.field_errors.len(),
                usize::from(!accepted),
                "published {count}"
            );
            assert_eq!(
                po.field_errors.len(),
                usize::from(!accepted),
                "opened {count}"
            );
        }
    }
}

mod number_bounds {
    use super::*;

    #[test]
    fn udp_port_and_local_are_1_to_65535() {
        for (n, accepted) in [(0u32, false), (1, true), (65535, true), (65536, false)] {
            // given
            let published =
                format!(r#"{{"published":[{{"port":{n},"proto":"udp","name":"","for":"all"}}]}}"#);
            let port = format!(
                r#"{{"opened":[{{"node":"{}","port":{n},"proto":"udp","local":9000}}]}}"#,
                id(1)
            );
            let local = format!(
                r#"{{"opened":[{{"node":"{}","port":53,"proto":"udp","local":{n}}}]}}"#,
                id(1)
            );
            // when
            let results = [parse(&published), parse(&port), parse(&local)];
            // then
            for p in results {
                let kept = p.settings.net.ports_published.len() + p.settings.net.ports_opened.len();
                assert_eq!(
                    kept,
                    usize::from(accepted),
                    "number {n}: {:?}",
                    p.field_errors
                );
            }
        }
    }
}

/// `count` publications, alternating TCP and UDP, every (protocol, port) distinct.
fn published_mixed(count: usize) -> String {
    let one = |i: usize| {
        let proto = Proto::ALL[i % 2].as_str();
        format!(
            r#"{{"port":{},"proto":"{proto}","name":"","for":"all"}}"#,
            1 + i / 2
        )
    };
    format!("[{}]", (0..count).map(one).collect::<Vec<_>>().join(","))
}

/// `count` opened ports, alternating TCP and UDP, every (protocol, local) distinct.
fn opened_mixed(count: usize) -> String {
    let one = |i: usize| {
        let proto = Proto::ALL[i % 2].as_str();
        format!(
            r#"{{"node":"{}","port":{},"proto":"{proto}","local":{}}}"#,
            id(1),
            1 + i / 2,
            2000 + i / 2
        )
    };
    format!("[{}]", (0..count).map(one).collect::<Vec<_>>().join(","))
}
