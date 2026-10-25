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
            assert_eq!(
                reply.header("Cache-Control").as_deref(),
                Some("no-store"),
                "{what}"
            );
        }
    }

    #[test]
    fn connection_is_close_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, false));
            assert_eq!(
                reply.header("Connection").as_deref(),
                Some("close"),
                "{what}"
            );
        }
    }

    #[test]
    fn content_length_is_the_body_size_in_bytes_for_every_variant() {
        for (what, page) in every_variant() {
            let reply = parse(&render_page(&page, false));
            let declared = reply
                .header("Content-Length")
                .expect("Content-Length is present");
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
        assert!(
            bytes.ends_with(b"\r\n\r\n"),
            "a HEAD answer stops at the blank line"
        );
    }
}

mod not_found {
    use super::*;

    mod named_address {
        use super::*;

        #[test]
        fn title_names_the_missing_member() {
            let body = body_of(&not_found("x", &[]));
            assert!(
                body.contains("There is no member called \u{201c}x\u{201d}"),
                "{body}"
            );
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
            assert!(
                !body.contains("<b>"),
                "raw markup from the label leaked: {body}"
            );
            assert!(body.contains("&lt;b&gt;&amp;"), "{body}");
        }

        #[test]
        fn nested_label_is_shown_as_is() {
            let body = body_of(&not_found("a.b", &[]));
            assert!(
                body.contains("There is no member called \u{201c}a.b\u{201d}"),
                "{body}"
            );
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
                body.contains(
                    "Did you mean <a href=\"http://box.meshtown\">http://box.meshtown</a>?"
                ),
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
                &[
                    "http://one.meshtown",
                    "http://two.meshtown",
                    "http://three.meshtown",
                ],
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

pub(super) struct Reply {
    /// Status line and headers, without the blank line.
    pub(super) head: String,
    pub(super) body: Vec<u8>,
}

impl Reply {
    pub(super) fn status_line(&self) -> &str {
        self.head.lines().next().unwrap_or_default()
    }

    pub(super) fn header(&self, name: &str) -> Option<String> {
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
    }
}

pub(super) fn parse(bytes: &[u8]) -> Reply {
    let split = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("answer has a blank line after the headers");
    Reply {
        head: String::from_utf8(bytes[..split].to_vec()).expect("head is UTF-8"),
        body: bytes[split + 4..].to_vec(),
    }
}

pub(super) fn body_of(page: &MeshPage) -> String {
    let reply = parse(&render_page(page, false));
    String::from_utf8(reply.body).expect("body is UTF-8")
}

pub(super) fn assert_status(page: &MeshPage, prefix: &str) {
    let reply = parse(&render_page(page, false));
    assert!(
        reply.status_line().starts_with(prefix),
        "status line is {:?}, expected it to start with {prefix:?}",
        reply.status_line()
    );
}

pub(super) fn make(kind: PageKind) -> MeshPage {
    MeshPage {
        me: "Cafe Laptop".to_string(),
        host: "router.meshtown".to_string(),
        kind,
    }
}

pub(super) fn open_ports(ports: &[(u16, &str)]) -> Vec<OpenPort> {
    ports
        .iter()
        .map(|(port, name)| OpenPort {
            port: *port,
            name: name.to_string(),
            url: format!("http://router.meshtown:{port}"),
        })
        .collect()
}

pub(super) fn not_found(label: &str, suggest: &[&str]) -> MeshPage {
    make(PageKind::NotFound {
        label: label.to_string(),
        suggest: suggest.iter().map(|s| s.to_string()).collect(),
    })
}

/// Candidates get addresses `box-0001.meshtown`, `box-0002.meshtown`, ... in the given order.
pub(super) fn ambiguous(candidates: &[(&str, bool)]) -> MeshPage {
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

pub(super) fn not_opened(open: &[(u16, &str)], offline: bool) -> MeshPage {
    make(PageKind::NotOpened {
        device: "Caf\u{e9} Router".to_string(),
        url: "http://router.meshtown:8080".to_string(),
        port: 8080,
        open: open_ports(open),
        offline,
    })
}

pub(super) fn offline(open: &[(u16, &str)]) -> MeshPage {
    make(PageKind::Offline {
        device: "Caf\u{e9} Router".to_string(),
        url: "http://router.meshtown:8080".to_string(),
        port: 8080,
        open: open_ports(open),
    })
}

pub(super) fn no_service(timed_out: bool) -> MeshPage {
    make(PageKind::NoService {
        device: "Caf\u{e9} Router".to_string(),
        url: "http://router.meshtown:8080".to_string(),
        port: 8080,
        timed_out,
    })
}

pub(super) fn every_variant() -> Vec<(&'static str, MeshPage)> {
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
