//! HTML answer pages.

use super::page_html::{self, Parts};
use super::page_texts::{PAGE_BRAND, PAGE_CSS, PAGE_SCRIPT};

/// A port the member has opened to this node.
pub(crate) struct OpenPort {
    pub port: u16,
    pub name: String,
    pub url: String,
}

/// One candidate of an ambiguous address.
pub(crate) struct MeshChoice {
    pub url: String,
    pub name: String,
    pub online: bool,
}

/// An answer page: who speaks, which host was asked, and the variant.
pub(crate) struct MeshPage {
    pub me: String,
    /// `<rest>.meshtown`, lower case, without trailing dots and port.
    pub host: String,
    pub kind: PageKind,
}

pub(crate) enum PageKind {
    NotFound { label: String, suggest: Vec<String> },
    Ambiguous { choices: Vec<MeshChoice> },
    NotOpened { device: String, url: String, port: u16, open: Vec<OpenPort>, offline: bool },
    Offline { device: String, url: String, port: u16, open: Vec<OpenPort> },
    NoService { device: String, url: String, port: u16, timed_out: bool },
}

/// Renders a complete HTTP/1.1 response; with `head_only` the body is left out.
pub(crate) fn render_page(page: &MeshPage, head_only: bool) -> Vec<u8> {
    let parts = parts_of(page);
    let html = document(&parts);
    let mut out = head(parts.status, html.len()).into_bytes();
    if !head_only {
        out.extend_from_slice(html.as_bytes());
    }
    out
}

fn parts_of(page: &MeshPage) -> Parts {
    match &page.kind {
        PageKind::NotFound { label, suggest } => page_html::not_found(page, label, suggest),
        PageKind::Ambiguous { choices } => page_html::ambiguous(page, choices),
        PageKind::NotOpened { device, url, port, open, offline } => {
            page_html::not_opened(page, device, url, *port, open, *offline)
        }
        PageKind::Offline { device, url, port, open } => {
            page_html::offline(page, device, url, *port, open)
        }
        PageKind::NoService { device, url, port, timed_out } => {
            page_html::no_service(page, device, url, *port, *timed_out)
        }
    }
}

fn reason(status: u16) -> &'static str {
    let known = [
        (300, "Multiple Choices"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (502, "Bad Gateway"),
        (503, "Service Unavailable"),
        (504, "Gateway Timeout"),
    ];
    known.iter().find(|(code, _)| *code == status).map_or("Error", |(_, text)| text)
}

fn head(status: u16, length: usize) -> String {
    format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n",
        reason(status)
    )
}

fn document(parts: &Parts) -> String {
    let title = &parts.title;
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{title}</title><style>{PAGE_CSS}</style></head><body><div class=\"brand\">{PAGE_BRAND}</div><div class=\"card\">{}<p id=\"copy-note\" class=\"hint\" role=\"status\" hidden></p></div><script>{PAGE_SCRIPT}</script></body></html>",
        parts.body
    )
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
