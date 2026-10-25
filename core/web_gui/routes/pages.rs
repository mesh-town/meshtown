//! Static pages of the node: one "path → type → gzip body" table
//! (`PAGES`, bodies gzip-compressed at build time); `register` builds the router routes from it too — a new file in
//! `core/web_gui/ui/` (for example, one more `core/web_gui/ui/css/*.css`) adds only a line here.

use std::sync::Arc;

use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use super::helpers::bad_method;
use super::page_encoding::{accepts_gzip, inflate_gzip};
use crate::web_gui::App;

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;

const HTML: &str = "text/html; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";

/// Embeds a page compressed by `core/build.rs` (`$OUT_DIR/ui/<path>.gz`).
macro_rules! gz {
    ($rel:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/ui/", $rel, ".gz"))
    };
}

/// (URL path, MIME type, gzip-compressed file content). The order of CSS entries is the order of
/// `<link>` in `core/web_gui/ui/index.html` (style cascade); the page is one built Vue module
/// `/bundle/app.js`, which comes last. The build is committed (`make web-build`), so
/// `cargo build` does without Node; `core/build.rs` compresses the files.
pub(super) const PAGES: &[(&str, &str, &[u8])] = &[
    ("/", HTML, gz!("index.html")),
    ("/css/00-base.css", CSS, gz!("css/00-base.css")),
    ("/css/10-shell.css", CSS, gz!("css/10-shell.css")),
    ("/css/20-home.css", CSS, gz!("css/20-home.css")),
    ("/css/30-layout.css", CSS, gz!("css/30-layout.css")),
    ("/css/40-forms.css", CSS, gz!("css/40-forms.css")),
    ("/css/50-sections.css", CSS, gz!("css/50-sections.css")),
    ("/css/60-narrow.css", CSS, gz!("css/60-narrow.css")),
    ("/css/70-extras.css", CSS, gz!("css/70-extras.css")),
    ("/css/80-nets.css", CSS, gz!("css/80-nets.css")),
    ("/bundle/app.js", JS, gz!("bundle/app.js")),
];

/// Answers with the embedded gzip body when the client accepts gzip, and with the inflated body
/// otherwise; both answers vary on `Accept-Encoding`.
async fn serve_page(
    headers: HeaderMap,
    content_type: &'static str,
    body_gz: &'static [u8],
) -> Response {
    let vary = (header::VARY, "Accept-Encoding");
    if accepts_gzip(&headers) {
        let encoding = (header::CONTENT_ENCODING, "gzip");
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type), encoding, vary],
            body_gz,
        )
            .into_response();
    }
    match inflate_gzip(body_gz) {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, content_type), vary],
            body,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Registers the routes of all `PAGES` entries (called from `register_pages`
/// in `mod.rs`); on disallowed methods every path answers `bad_method`,
/// like the other routes of the router.
pub(super) fn register(router: Router<Arc<App>>) -> Router<Arc<App>> {
    let mut router = router;
    for &(path, content_type, body) in PAGES {
        router = router.route(
            path,
            get(move |headers: HeaderMap| serve_page(headers, content_type, body))
                .fallback(bad_method),
        );
    }
    router
}
