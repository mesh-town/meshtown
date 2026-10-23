use super::tests::*;
use super::*;

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
        fn copy_button_label_is_escaped_once() {
            let mut page = not_opened(&[], false);
            if let PageKind::NotOpened { device, .. } = &mut page.kind {
                *device = "A&B".to_string();
            }
            let body = body_of(&page);
            assert!(body.contains("aria-label=\"Copy message for the owner of A&amp;B\""), "{body}");
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
