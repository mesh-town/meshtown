//! Sign-in by token: the status probe and the token exchange `/api/auth/token`.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::{acquire_attempt_permit, body_string_field, open_session_response, record_attempt};
use crate::web_gui::auth::tickets::Via;
use crate::web_gui::guard::{ct_eq, Credential};
use crate::web_gui::http::{json_err, ConnInfo};
use crate::web_gui::App;

pub(in crate::web_gui::routes) async fn status_handler(
    State(app): State<Arc<App>>,
    cred: Option<Extension<Credential>>,
) -> Response {
    let auth = &app.shared.auth;
    let via = cred.map(|Extension(c)| c.via.as_str());
    let body = json!({
        "password_set": auth.password_set(),
        "access": auth.access().as_str(),
        "listening": auth.bound_access().as_str(),
        "via": via,
    });
    (StatusCode::OK, axum::Json(body)).into_response()
}

/// A temporary token is invalidated on first presentation; otherwise it is the permanent one.
fn redeem_token(app: &App, candidate: &str) -> Option<Via> {
    let temp = {
        let mut tickets = app
            .shared
            .auth
            .tickets
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tickets.take_temp(candidate, Instant::now())
    };
    if temp {
        return Some(Via::TempToken);
    }
    matches_permanent_token(app, candidate).then_some(Via::PermanentToken)
}

/// Constant-time comparison of the candidate with the permanent token.
fn matches_permanent_token(app: &App, candidate: &str) -> bool {
    let real = app.shared.token();
    ct_eq(candidate.as_bytes(), real.as_bytes())
}

pub(in crate::web_gui::routes) async fn token_handler(
    State(app): State<Arc<App>>,
    Extension(conn): Extension<ConnInfo>,
    body: Bytes,
) -> Response {
    let candidate = match body_string_field(&body, "token") {
        Ok(t) => t,
        Err(resp) => return resp,
    };
    let ip = conn.client_addr.ip();
    let _permit = match acquire_attempt_permit(&app, ip).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let via = redeem_token(&app, &candidate);
    record_attempt(&app, ip, via.is_some());
    match via {
        Some(via) => open_session_response(&app, via),
        None => json_err(StatusCode::UNAUTHORIZED, "bad_token", "token not accepted"),
    }
}
