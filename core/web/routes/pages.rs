//! Static pages of the node: one "path → type → `include_str!`" table
//! (`PAGES`); `register` builds the router routes from it too — a new file in
//! `core/web_gui/ui/` (for example, one more `core/web_gui/ui/css/*.css`) adds only a line here.

use std::sync::Arc;

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use super::helpers::bad_method;
use crate::web::App;

#[cfg(test)]
#[path = "pages_tests.rs"]
mod tests;

const HTML: &str = "text/html; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";

/// (URL path, MIME type, file content). The order of CSS entries is the order of
/// `<link>` in `core/web_gui/ui/index.html` (style cascade); the page is one built Vue module
/// `/bundle/app.js`, which comes last. The build is committed (`make web-build`), so
/// `cargo build` does without Node.
const PAGES: &[(&str, &str, &str)] = &[
    ("/", HTML, include_str!("../../web_gui/ui/index.html")),
    ("/css/00-base.css", CSS, include_str!("../../web_gui/ui/css/00-base.css")),
    ("/css/10-shell.css", CSS, include_str!("../../web_gui/ui/css/10-shell.css")),
    ("/css/20-home.css", CSS, include_str!("../../web_gui/ui/css/20-home.css")),
    ("/css/30-layout.css", CSS, include_str!("../../web_gui/ui/css/30-layout.css")),
    ("/css/40-forms.css", CSS, include_str!("../../web_gui/ui/css/40-forms.css")),
    ("/css/50-sections.css", CSS, include_str!("../../web_gui/ui/css/50-sections.css")),
    ("/css/60-narrow.css", CSS, include_str!("../../web_gui/ui/css/60-narrow.css")),
    ("/css/70-extras.css", CSS, include_str!("../../web_gui/ui/css/70-extras.css")),
    ("/css/80-nets.css", CSS, include_str!("../../web_gui/ui/css/80-nets.css")),
    ("/bundle/app.js", JS, include_str!("../../web_gui/ui/bundle/app.js")),
];

async fn serve_page(content_type: &'static str, body: &'static str) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        body,
    )
        .into_response()
}

/// Registers the routes of all `PAGES` entries (called from `register_pages`
/// in `mod.rs`); on disallowed methods every path answers `bad_method`,
/// like the other routes of the router.
pub(super) fn register(router: Router<Arc<App>>) -> Router<Arc<App>> {
    let mut router = router;
    for &(path, content_type, body) in PAGES {
        router = router.route(
            path,
            get(move || serve_page(content_type, body)).fallback(bad_method),
        );
    }
    router
}
