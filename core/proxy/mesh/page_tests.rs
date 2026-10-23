use super::*;

mod status_line {
    use super::*;

    #[test]
    fn not_found_is_404() {
        assert_status(&not_found("box", &[]), "HTTP/1.1 404 ");
    }

    #[test]
    fn ambiguous_is_300() {
        assert_status(&ambiguous(&[("Box", true), ("Box", true)]), "HTTP/1.1 300 ");
    }

    #[test]
    fn not_opened_is_403() {
        assert_status(&not_opened(&[], false), "HTTP/1.1 403 ");
    }

    #[test]
    fn offline_is_503() {
        assert_status(&offline(&[]), "HTTP/1.1 503 ");
    }

    #[test]
    fn no_service_is_502() {
        assert_status(&no_service(false), "HTTP/1.1 502 ");
    }

    #[test]
    fn no_service_that_timed_out_is_504() {
        assert_status(&no_service(true), "HTTP/1.1 504 ");
    }
}

mod headers {
    use super::*;

    #[test]
    fn content_type_is_html_utf8_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, false));
            assert_eq!(
                reply.header("Content-Type").as_deref(),
                Some("text/html; charset=utf-8"),
                "{what}"
            );
        }
    }

    #[test]
    fn cache_control_is_no_store_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, false));
            assert_eq!(reply.header("Cache-Control").as_deref(), Some("no-store"), "{what}");
        }
    }

    #[test]
    fn connection_is_close_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, false));
            assert_eq!(reply.header("Connection").as_deref(), Some("close"), "{what}");
        }
    }

    #[test]
    fn content_length_is_the_body_size_in_bytes_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, false));
            let declared = reply.header("Content-Length").expect("Content-Length is present");
            assert_eq!(declared, reply.body.len().to_string(), "{what}");
        }
    }

    #[test]
    fn body_is_not_empty_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, false));
            assert!(!reply.body.is_empty(), "{what}");
        }
    }
}

mod head_only {
    use super::*;

    #[test]
    fn has_no_body_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, true));
            assert!(reply.body.is_empty(), "{what}: HEAD answer carries a body");
        }
    }

    #[test]
    fn keeps_the_content_length_of_the_full_page_for_every_variant() {
        for (what, page) in every_variant() {
            let full = parse(&render_page(&page, false));
            let head = parse(&render_page(&page, true));
            assert_eq!(
                head.header("Content-Length"),
                Some(full.body.len().to_string()),
                "{what}"
            );
        }
    }

    #[test]
    fn keeps_the_status_line_and_headers_of_the_full_page_for_every_variant() {
        for (what, page) in every_variant() {
            let full = parse(&render_page(&page, false));
            let head = parse(&render_page(&page, true));
            assert_eq!(head.head, full.head, "{what}");
        }
    }

    #[test]
    fn ends_right_after_the_blank_line() {
        let bytes = render_page(&no_service(true), true);
        assert!(bytes.ends_with(b"\r\n\r\n"), "a HEAD answer stops at the blank line");
    }
}

mod not_found {
    use super::*;

    mod named_address {
        use super::*;

        #[test]
        fn title_names_the_missing_member() {
            let body = body_of(&not_found("x", &[]));
            assert!(body.contains("There is no member called \u{201c}x\u{201d}"), "{body}");
        }

        #[test]
        fn says_nothing_matched_in_your_networks() {
            let body = body_of(&not_found("x", &[]));
            assert!(
                body.contains("didn&#39;t find a member with this name in your networks"),
                "{body}"
            );
        }

        #[test]
        fn label_is_shown_escaped() {
            let body = body_of(&not_found("<b>&\"", &[]));
            assert!(!body.contains("<b>"), "raw markup from the label leaked: {body}");
            assert!(body.contains("&lt;b&gt;&amp;"), "{body}");
        }

        #[test]
        fn nested_label_is_shown_as_is() {
            let body = body_of(&not_found("a.b", &[]));
            assert!(body.contains("There is no member called \u{201c}a.b\u{201d}"), "{body}");
        }

        #[test]
        fn has_no_try_again_button() {
            let body = body_of(&not_found("x", &[]));
            assert!(!body.contains("Try again"), "{body}");
        }

        #[test]
        fn names_who_is_speaking() {
            let body = body_of(&not_found("x", &[]));
            assert!(body.contains("from Mesh Town on Cafe Laptop"), "{body}");
        }
    }

    mod empty_label {
        use super::*;

        #[test]
        fn title_says_the_address_has_no_member_name() {
            let body = body_of(&not_found("", &[]));
            assert!(body.contains("This address has no member name"), "{body}");
        }

        #[test]
        fn does_not_quote_an_empty_name() {
            let body = body_of(&not_found("", &[]));
            assert!(!body.contains("There is no member called"), "{body}");
        }
    }

    mod suggestions {
        use super::*;

        #[test]
        fn none_means_no_did_you_mean() {
            let body = body_of(&not_found("x", &[]));
            assert!(!body.contains("Did you mean"), "{body}");
        }

        #[test]
        fn one_is_offered_as_a_link() {
            let body = body_of(&not_found("x", &["http://box.meshtown"]));
            assert!(
                body.contains("Did you mean <a href=\"http://box.meshtown\">http://box.meshtown</a>?"),
                "{body}"
            );
        }

        #[test]
        fn one_has_no_separator() {
            let body = body_of(&not_found("x", &["http://box.meshtown"]));
            assert!(!body.contains("</a> or <a"), "{body}");
        }

        #[test]
        fn several_are_joined_with_or_in_order() {
            let body = body_of(&not_found(
                "x",
                &["http://one.meshtown", "http://two.meshtown", "http://three.meshtown"],
            ));
            let expected = concat!(
                "Did you mean <a href=\"http://one.meshtown\">http://one.meshtown</a>",
                " or <a href=\"http://two.meshtown\">http://two.meshtown</a>",
                " or <a href=\"http://three.meshtown\">http://three.meshtown</a>?"
            );
            assert!(body.contains(expected), "{body}");
        }
    }
}

mod ambiguous {
    use super::*;

    #[test]
    fn title_counts_the_matching_members_for_the_host() {
        let body = body_of(&ambiguous(&[("Box", true), ("Box", false), ("Box", true)]));
        assert!(body.contains("3 members match router.meshtown"), "{body}");
    }

    #[test]
    fn lists_every_candidate_address_and_name() {
        let body = body_of(&ambiguous(&[("Box One", true), ("Box Two", true)]));
        for needle in ["http://box-0001.meshtown", "http://box-0002.meshtown", "Box One", "Box Two"] {
            assert!(body.contains(needle), "missing {needle}: {body}");
        }
    }

    #[test]
    fn keeps_the_order_given_by_the_caller() {
        let body = body_of(&ambiguous(&[("Box One", true), ("Box Two", true)]));
        let first = body.find("http://box-0001.meshtown").expect("first address");
        let second = body.find("http://box-0002.meshtown").expect("second address");
        assert!(first < second, "candidates are not in the given order");
    }

    #[test]
    fn online_candidate_gets_the_online_badge() {
        let body = body_of(&ambiguous(&[("Box", true), ("Box", true)]));
        assert!(body.contains("online"), "{body}");
    }

    #[test]
    fn online_candidates_alone_leave_no_offline_badge() {
        let body = body_of(&ambiguous(&[("Box", true), ("Box", true)]));
        assert!(!body.contains("offline"), "{body}");
    }

    #[test]
    fn offline_candidate_gets_the_offline_badge() {
        let body = body_of(&ambiguous(&[("Box", true), ("Box", false)]));
        assert!(body.contains("offline"), "{body}");
    }

    #[test]
    fn candidate_name_is_shown_escaped() {
        let body = body_of(&ambiguous(&[("<b>&\"", true), ("Box", true)]));
        assert!(!body.contains("<b>"), "raw markup from the name leaked: {body}");
        assert!(body.contains("&lt;b&gt;&amp;"), "{body}");
    }

    #[test]
    fn has_no_try_again_button() {
        let body = body_of(&ambiguous(&[("Box", true), ("Box", true)]));
        assert!(!body.contains("Try again"), "{body}");
    }
}

mod not_opened {
    use super::*;

    mod requested_port {
        use super::*;

        #[test]
        fn is_named_in_the_page() {
            let body = body_of(&not_opened(&[], false));
            assert!(body.contains("hasn't shared port 8080 with you"), "{body}");
        }

        #[test]
        fn is_named_with_the_device() {
            let body = body_of(&not_opened(&[], false));
            assert!(body.contains("Caf\u{e9} Router hasn't shared port 8080"), "{body}");
        }
    }

    mod one_open_port {
        use super::*;

        #[test]
        fn becomes_an_open_button_with_its_url() {
            let body = body_of(&not_opened(&[(3000, "web")], false));
            assert!(body.contains("href=\"http://router.meshtown:3000\""), "{body}");
            assert!(body.contains("Open port 3000"), "{body}");
        }

        #[test]
        fn button_shows_the_port_name() {
            let body = body_of(&not_opened(&[(3000, "web")], false));
            assert!(body.contains("Open port 3000 (web)"), "{body}");
        }

        #[test]
        fn nameless_port_gets_a_button_without_parentheses() {
            let body = body_of(&not_opened(&[(3000, "")], false));
            assert!(body.contains("Open port 3000"), "{body}");
            assert!(!body.contains("Open port 3000 ("), "{body}");
        }
    }

    mod several_open_ports {
        use super::*;

        #[test]
        fn every_port_url_is_listed() {
            let body = body_of(&not_opened(&[(3000, "web"), (5000, "api")], false));
            assert!(body.contains("http://router.meshtown:3000"), "{body}");
            assert!(body.contains("http://router.meshtown:5000"), "{body}");
        }

        #[test]
        fn every_port_name_is_listed() {
            let body = body_of(&not_opened(&[(3000, "web"), (5000, "api")], false));
            assert!(body.contains("web") && body.contains("api"), "{body}");
        }

        #[test]
        fn heading_says_what_was_shared() {
            let body = body_of(&not_opened(&[(3000, "web"), (5000, "api")], false));
            assert!(body.contains("shared with you</h2>"), "{body}");
        }

        #[test]
        fn there_is_no_single_open_button() {
            let body = body_of(&not_opened(&[(3000, "web"), (5000, "api")], false));
            assert!(!body.contains("Open port"), "{body}");
        }
    }

    mod no_open_ports {
        use super::*;

        #[test]
        fn says_nothing_was_shared_yet() {
            let body = body_of(&not_opened(&[], false));
            assert!(body.contains("hasn't shared any ports with you yet"), "{body}");
        }

        #[test]
        fn has_no_open_button() {
            let body = body_of(&not_opened(&[], false));
            assert!(!body.contains("Open port"), "{body}");
        }
    }

    mod some_open_ports {
        use super::*;

        #[test]
        fn do_not_claim_that_nothing_was_shared() {
            let body = body_of(&not_opened(&[(3000, "web")], false));
            assert!(!body.contains("hasn't shared any ports"), "{body}");
        }
    }

    mod owner_is_offline {
        use super::*;

        #[test]
        fn page_adds_the_also_offline_line() {
            let body = body_of(&not_opened(&[], true));
            assert!(body.contains("is also offline right now"), "{body}");
        }

        #[test]
        fn online_owner_gets_no_such_line() {
            let body = body_of(&not_opened(&[], false));
            assert!(!body.contains("offline"), "{body}");
        }
    }

    mod retry {
        use super::*;

        #[test]
        fn try_again_leads_to_the_requested_url() {
            let body = body_of(&not_opened(&[], false));
            assert!(body.contains("<a class=\"btn\" href=\"http://router.meshtown:8080\">Try again</a>"), "{body}");
        }
    }

    mod owner_instructions {
        use super::*;

        #[test]
        fn ask_to_share_the_requested_port_and_name_the_member() {
            let body = body_of(&not_opened(&[], false));
            assert!(body.contains("share port 8080 with you"), "{body}");
            assert!(body.contains("tick Cafe Laptop"), "{body}");
        }

        #[test]
        fn message_for_the_owner_is_offered_for_copying() {
            let body = body_of(&not_opened(&[], false));
            assert!(body.contains("Copy message"), "{body}");
        }
    }

    mod escaping {
        use super::*;

        #[test]
        fn device_name_is_shown_escaped() {
            let mut page = not_opened(&[], false);
            if let PageKind::NotOpened { device, .. } = &mut page.kind {
                *device = "<b>&\"".to_string();
            }
            let body = body_of(&page);
            assert!(!body.contains("<b>"), "raw markup from the name leaked: {body}");
            assert!(body.contains("&lt;b&gt;&amp;"), "{body}");
        }

        #[test]
        fn own_name_is_shown_escaped() {
            let mut page = not_opened(&[], false);
            page.me = "<b>&\"".to_string();
            let body = body_of(&page);
            assert!(!body.contains("<b>"), "raw markup from the name leaked: {body}");
        }

        #[test]
        fn open_port_name_is_shown_escaped() {
            let body = body_of(&not_opened(&[(3000, "<b>&\"")], false));
            assert!(!body.contains("<b>"), "raw markup from the port name leaked: {body}");
        }
    }
}

mod offline {
    use super::*;

    #[test]
    fn title_says_the_device_is_offline() {
        let body = body_of(&offline(&[]));
        assert!(body.contains("Caf\u{e9} Router is offline"), "{body}");
    }

    #[test]
    fn promises_to_open_once_it_is_back() {
        let body = body_of(&offline(&[]));
        assert!(body.contains("shared port 8080 with you"), "{body}");
        assert!(body.contains("as soon as"), "{body}");
    }

    #[test]
    fn has_a_try_again_button_to_the_requested_url() {
        let body = body_of(&offline(&[]));
        assert!(body.contains("href=\"http://router.meshtown:8080\">Try again</a>"), "{body}");
    }

    #[test]
    fn other_open_ports_are_listed_with_their_urls() {
        let body = body_of(&offline(&[(3000, "web")]));
        assert!(body.contains("Other ports"), "{body}");
        assert!(body.contains("http://router.meshtown:3000"), "{body}");
    }

    #[test]
    fn no_other_ports_means_no_list() {
        let body = body_of(&offline(&[]));
        assert!(!body.contains("Other ports"), "{body}");
    }
}

mod no_service {
    use super::*;

    #[test]
    fn plain_failure_says_no_program_accepted_the_connection() {
        let body = body_of(&no_service(false));
        assert!(body.contains("no program on port 8080 accepted the connection"), "{body}");
    }

    #[test]
    fn timeout_says_the_port_did_not_answer_in_time() {
        let body = body_of(&no_service(true));
        assert!(body.contains("port 8080 didn't answer in time"), "{body}");
    }

    #[test]
    fn timeout_does_not_use_the_refusal_text() {
        let body = body_of(&no_service(true));
        assert!(!body.contains("accepted the connection"), "{body}");
    }

    #[test]
    fn title_names_the_port_and_the_device() {
        let body = body_of(&no_service(false));
        assert!(body.contains("Nothing answers on port 8080 of Caf\u{e9} Router"), "{body}");
    }

    #[test]
    fn has_a_try_again_button_to_the_requested_url() {
        let body = body_of(&no_service(false));
        assert!(body.contains("href=\"http://router.meshtown:8080\">Try again</a>"), "{body}");
    }

    #[test]
    fn lists_no_open_ports() {
        let body = body_of(&no_service(false));
        assert!(!body.contains("shared with you</h2>"), "{body}");
    }
}

mod common_text {
    use super::*;

    #[test]
    fn never_mentions_last_seen_in_any_variant() {
        for (what, page) in every_variant() {
            let body = body_of(&page).to_lowercase();
            assert!(!body.contains("last seen"), "{what}");
        }
    }

    #[test]
    fn says_the_page_is_from_mesh_town_on_this_member() {
        for (what, page) in every_variant() {
            let body = body_of(&page);
            assert!(
                body.contains("This page is from Mesh Town on Cafe Laptop, not from router.meshtown."),
                "{what}"
            );
        }
    }

    #[test]
    fn is_an_html_document() {
        for (what, page) in every_variant() {
            let body = body_of(&page);
            assert!(body.contains("<html lang=\"en\">"), "{what}");
        }
    }
}

struct Reply {
    /// Status line and headers, without the blank line.
    head: String,
    body: Vec<u8>,
}

impl Reply {
    fn status_line(&self) -> &str {
        self.head.lines().next().unwrap_or_default()
    }

    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name).then(|| value.trim().to_string())
        })
    }
}

fn parse(bytes: &[u8]) -> Reply {
    let split = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("answer has a blank line after the headers");
    Reply {
        head: String::from_utf8(bytes[..split].to_vec()).expect("head is UTF-8"),
        body: bytes[split + 4..].to_vec(),
    }
}

fn body_of(page: &MeshPage) -> String {
    let reply = parse(&render_page(page, false));
    String::from_utf8(reply.body).expect("body is UTF-8")
}

fn assert_status(page: &MeshPage, prefix: &str) {
    let reply = parse(&render_page(page, false));
    assert!(
        reply.status_line().starts_with(prefix),
        "status line is {:?}, expected it to start with {prefix:?}",
        reply.status_line()
    );
}

fn make(kind: PageKind) -> MeshPage {
    MeshPage {
        me: "Cafe Laptop".to_string(),
        host: "router.meshtown".to_string(),
        kind,
    }
}

fn open_ports(ports: &[(u16, &str)]) -> Vec<OpenPort> {
    ports
        .iter()
        .map(|(port, name)| OpenPort {
            port: *port,
            name: name.to_string(),
            url: format!("http://router.meshtown:{port}"),
        })
        .collect()
}

fn not_found(label: &str, suggest: &[&str]) -> MeshPage {
    make(PageKind::NotFound {
        label: label.to_string(),
        suggest: suggest.iter().map(|s| s.to_string()).collect(),
    })
}

/// Candidates get addresses `box-0001.meshtown`, `box-0002.meshtown`, ... in the given order.
fn ambiguous(candidates: &[(&str, bool)]) -> MeshPage {
    let choices = candidates
        .iter()
        .enumerate()
        .map(|(i, (name, online))| MeshChoice {
            url: format!("http://box-{:04}.meshtown", i + 1),
            name: name.to_string(),
            online: *online,
        })
        .collect();
    make(PageKind::Ambiguous { choices })
}

fn not_opened(open: &[(u16, &str)], offline: bool) -> MeshPage {
    make(PageKind::NotOpened {
        device: "Caf\u{e9} Router".to_string(),
        url: "http://router.meshtown:8080".to_string(),
        port: 8080,
        open: open_ports(open),
        offline,
    })
}

fn offline(open: &[(u16, &str)]) -> MeshPage {
    make(PageKind::Offline {
        device: "Caf\u{e9} Router".to_string(),
        url: "http://router.meshtown:8080".to_string(),
        port: 8080,
        open: open_ports(open),
    })
}

fn no_service(timed_out: bool) -> MeshPage {
    make(PageKind::NoService {
        device: "Caf\u{e9} Router".to_string(),
        url: "http://router.meshtown:8080".to_string(),
        port: 8080,
        timed_out,
    })
}

fn every_variant() -> Vec<(&'static str, MeshPage)> {
    vec![
        ("not found", not_found("x", &["http://box.meshtown"])),
        ("not found, empty label", not_found("", &[])),
        ("ambiguous", ambiguous(&[("Box", true), ("Box", false)])),
        ("not opened", not_opened(&[(3000, "web")], false)),
        ("not opened, owner offline", not_opened(&[], true)),
        ("offline", offline(&[(3000, "web")])),
        ("no service", no_service(false)),
        ("no service, timed out", no_service(true)),
    ]
}
