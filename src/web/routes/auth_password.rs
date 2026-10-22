//! `PUT /api/auth/password`: set or change the password. The old password is not asked for;
//! other sessions and all live temporary tokens expire.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::auth::{acquire_hash_permit, body_string_field};
use crate::web::auth::{password, password_file};
use crate::web::guard::Credential;
use crate::web::http::json_err;
use crate::web::App;

fn internal_error_response(text: &str) -> Response {
    json_err(StatusCode::INTERNAL_SERVER_ERROR, "internal", text)
}

/// Hashes in `spawn_blocking` under `hash_gate` and writes the file.
async fn store(app: &App, dir: std::path::PathBuf, pw: String) -> Result<(), String> {
    let _permit = acquire_hash_permit(app).await?;
    let work = move || password::hash(&pw).and_then(|r| password_file::write_password_file(&dir, &r).map_err(|e| e.to_string()));
    tokio::task::spawn_blocking(work).await.map_err(|e| e.to_string())?
}

fn revoke_other_logins(app: &App, keep_session_id: Option<&str>) {
    let mut tickets = app.shared.auth.tickets.lock().unwrap_or_else(|e| e.into_inner());
    tickets.close_sessions_except(keep_session_id);
    tickets.drop_temp_tokens();
}

pub(super) async fn password_put_handler(
    State(app): State<Arc<App>>,
    cred: Option<Extension<Credential>>,
    body: Bytes,
) -> Response {
    let pw = match body_string_field(&body, "password") {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    if let Err(text) = password::check_format(&pw) {
        return json_err(StatusCode::BAD_REQUEST, "bad_password_format", &text);
    }
    let Some(dir) = app.shared.auth.password_dir.clone() else {
        return json_err(StatusCode::CONFLICT, "no_storage", "nowhere to save the password");
    };
    if let Err(e) = store(&app, dir, pw).await {
        return internal_error_response(&format!("password not saved: {e}"));
    }
    let own = cred.and_then(|Extension(c)| c.session_id);
    revoke_other_logins(&app, own.as_deref());
    (StatusCode::OK, axum::Json(json!({"password_set": true}))).into_response()
}
