//! Changing the page token: `/api/token/rotate` (not the `token.rs` file, to avoid confusion with
//! `web::token`).

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::web::guard::Credential;
use crate::web::http::json_err;
use crate::web::token;
use crate::web::App;

pub(super) async fn token_rotate_handler(
    State(app): State<Arc<App>>,
    cred: Option<Extension<Credential>>,
) -> Response {
    let _cfg = app.config.lock().unwrap_or_else(|e| e.into_inner());
    let new_token = match token::generate() {
        Ok(t) => t,
        Err(e) => {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                &format!("token not changed: {e}"),
            )
        }
    };
    if let Some(dir) = app.token_dir.as_deref() {
        if let Err(e) = token::write_atomic(dir, &new_token) {
            return json_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                &format!("token not changed: {e}"),
            );
        }
    }
    app.shared.set_token(new_token.clone());
    let own = cred.and_then(|Extension(c)| c.session_id);
    let mut tickets = app.shared.auth.tickets.lock().unwrap_or_else(|e| e.into_inner());
    tickets.close_sessions_except(own.as_deref());
    drop(tickets);
    let body = json!({"token": new_token, "url": app.shared.open_url()});
    (StatusCode::OK, axum::Json(body)).into_response()
}
