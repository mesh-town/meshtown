//! Tests for serving static pages: the entry for the built `bundle/app.js` and the `serve_page` response.

use axum::body::to_bytes;
use axum::http::{header, StatusCode};

use super::{serve_page, CSS, JS, PAGES};

mod bundle_app_js {
    use super::*;

    #[test]
    fn is_listed_in_pages_as_javascript() {
        // given
        let entry = page_entry("/bundle/app.js");
        // then
        assert_eq!(entry.1, JS, "type of the /bundle/app.js entry");
        assert_eq!(JS, "text/javascript; charset=utf-8");
    }

    #[test]
    fn body_is_the_file_on_disk() {
        // given
        let entry = page_entry("/bundle/app.js");
        // then
        assert_eq!(
            entry.2,
            bundle_on_disk(),
            "the entry body differs from web_gui/ui/bundle/app.js"
        );
        assert!(!entry.2.is_empty(), "the built file is empty");
    }

    #[tokio::test]
    async fn is_served_with_200_and_javascript_type() {
        // given
        let (_, content_type, body) = page_entry("/bundle/app.js");
        // when
        let resp = serve_page(content_type, body).await;
        // then
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript; charset=utf-8"
        );
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(bytes.as_ref(), bundle_on_disk().as_bytes());
    }
}

mod pages_table {
    use super::*;

    #[test]
    fn no_classic_scripts_are_served() {
        // given: the page is one Vue module, there are no classic scripts
        let classic: Vec<_> = PAGES.iter().filter(|e| e.0.starts_with("/js/")).collect();
        // then
        assert!(
            classic.is_empty(),
            "classic scripts remain: {:?}",
            classic.iter().map(|e| e.0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn every_css_link_is_served() {
        // given: `<link href="css/…">` from index.html and the `/css/…` entries in PAGES are one set
        let html = page_entry("/").2;
        let linked: Vec<String> = html
            .split("href=\"css/")
            .skip(1)
            .map(|t| format!("/css/{}", t.split('"').next().unwrap()))
            .collect();
        let served: Vec<String> = PAGES
            .iter()
            .filter(|e| e.0.starts_with("/css/"))
            .inspect(|e| assert_eq!(e.1, CSS, "type {}", e.0))
            .map(|e| e.0.to_string())
            .collect();
        // then: the order matches too (cascade)
        assert!(!linked.is_empty(), "index.html has no links to css/");
        assert_eq!(
            linked, served,
            "the links in index.html and the PAGES entries diverge"
        );
    }
}

fn page_entry(path: &str) -> (&'static str, &'static str, &'static str) {
    *PAGES
        .iter()
        .find(|e| e.0 == path)
        .unwrap_or_else(|| panic!("PAGES has no entry for {path}"))
}

fn bundle_on_disk() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/web_gui/ui/bundle/app.js");
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}
