//! `/api/log`, `/api/log/download`: the live log and its download. `?net=<id>` limits the lines to one network.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use super::state::round1;
use crate::web_gui::http::query_param;
use crate::web_gui::{App, LogLine};

/// One line as JSON; `net` is added only when the label is shown.
fn line_json(l: &LogLine) -> Value {
    let mut v = json!({
        "seq": l.seq,
        "t": round1(l.t),
        "tag": l.tag,
        "text": l.text,
    });
    if let (true, Some(id), Some(obj)) = (l.shown, &l.net, v.as_object_mut()) {
        obj.insert("net".to_string(), json!(&**id));
    }
    v
}

/// One line of the downloaded text; the `[<id>] ` prefix only on lines whose label is shown.
fn line_text(l: &LogLine) -> String {
    let prefix = match (l.shown, &l.net) {
        (true, Some(id)) => format!("[{id}] "),
        _ => String::new(),
    };
    format!("+{:>7.1}s [{}] {}{}\n", l.t, l.tag, prefix, l.text)
}

pub(super) async fn log_handler(State(app): State<Arc<App>>, uri: axum::http::Uri) -> Response {
    let slot = app.host.main_slot();
    let after = query_param(&uri, "after")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let limit = query_param(&uri, "limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(500)
        .clamp(1, 2000);
    let net = query_param(&uri, "net");
    let page = slot.node.log_after(after, limit, net.as_deref());
    let gap = page.first_seq > 0 && after + 1 < page.first_seq;
    let lines: Vec<Value> = page.lines.iter().map(line_json).collect();
    let body = json!({
        "boot_id": page.boot_id,
        "first_seq": page.first_seq,
        "last_seq": page.last_seq,
        "gap": gap,
        "lines": lines,
    });
    (StatusCode::OK, axum::Json(body)).into_response()
}

pub(super) async fn log_download_handler(State(app): State<Arc<App>>) -> Response {
    let slot = app.host.main_slot();
    let page = slot.node.log_after(0, usize::MAX, None);
    let body: String = page.lines.iter().map(line_text).collect();
    let filename = format!("meshtown-{}.log", page.boot_id);
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            ),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        body,
    )
        .into_response()
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
