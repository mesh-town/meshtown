//! Tests of the step-by-step domain check without the network (`probe.rs`).

use super::*;
use crate::reachability::CheckVerdict;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::test]
async fn blocked_domain_is_network_blocked_without_network() {
    let cfg = CheckConfig {
        blocked_domains: Arc::new(["blocked.test".to_string()].into_iter().collect()),
        ..CheckConfig::default()
    };
    // The resolver points nowhere: if the check goes to the network, the test will notice.
    let resolver = Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        cfg.limits,
        cfg.user_agent.clone(),
    )
    .unwrap();
    let record = check_domain(&cfg, &resolver, "blocked.test").await.unwrap();
    assert_eq!(record.verdict, CheckVerdict::NetworkBlocked);
    assert_eq!(record.reason, "domain is marked --block-domain");
    assert_eq!(record.dns_ms, 0);
}

mod region_stub_marks {
    use super::*;

    mod built_in_phrases {
        use super::*;

        #[test]
        fn english_stub_is_geo() {
            // given: a page with the built-in English phrase and no extra phrases
            let resp = response("Sorry. Not available in your country");
            // when
            let mark = body_mark(&resp, &[]);
            // then
            assert_eq!(mark, BodyMark::Geo);
        }

        #[test]
        fn are_matched_ignoring_case() {
            let resp = response("<h1>GEO-RESTRICTED CONTENT</h1>");
            assert_eq!(body_mark(&resp, &[]), BodyMark::Geo);
        }

        #[test]
        fn still_work_when_extra_phrases_are_set() {
            let resp = response("This video is geo-restricted");
            let mark = body_mark(&resp, &phrases(&["unrelated phrase"]));
            assert_eq!(mark, BodyMark::Geo);
        }
    }

    mod extra_phrases_from_settings {
        use super::*;

        #[test]
        fn are_matched_ignoring_case_by_unicode_rules() {
            // given: a lowercase marker with non-ASCII letters and a page in uppercase
            let extra = phrases(&["ünavailable hère"]);
            let resp = response("ÜNAVAILABLE HÈRE");
            // when
            let mark = body_mark(&resp, &extra);
            // then
            assert_eq!(mark, BodyMark::Geo);
        }

        #[test]
        fn are_matched_inside_a_longer_page() {
            let extra = phrases(&["blocked for your region"]);
            let resp = response("<html><body><p>This site is Blocked For Your Region.</p></body></html>");
            assert_eq!(body_mark(&resp, &extra), BodyMark::Geo);
        }

        #[test]
        fn any_one_of_several_is_enough() {
            let extra = phrases(&["first phrase", "second phrase", "third phrase"]);
            let resp = response("only the Second Phrase is on this page");
            assert_eq!(body_mark(&resp, &extra), BodyMark::Geo);
        }

        #[test]
        fn absent_from_the_page_leave_it_clean() {
            let extra = phrases(&["ünavailable hère"]);
            let resp = response("A perfectly ordinary page");
            assert_eq!(body_mark(&resp, &extra), BodyMark::Clean);
        }
    }

    mod phrase_outside_the_lists {
        use super::*;

        #[test]
        fn is_clean_without_settings() {
            // given: a stub wording that is neither built in nor set in the settings
            let resp = response("This service is blocked for visitors from your region");
            // when
            let mark = body_mark(&resp, &[]);
            // then
            assert_eq!(mark, BodyMark::Clean);
        }
    }
}

fn response(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![],
        body: body.as_bytes().to_vec(),
        raw_bytes: body.len(),
        stalled: false,
        eof: true,
    }
}

fn phrases(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}
