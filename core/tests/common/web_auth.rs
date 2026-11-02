//! Helpers for the sign-in tests: the password the tests use, the permanent token file and a password
//! file in the on-disk format with a small number of iterations. Requests with a token exchange or a cookie
//! live in `web_auth_session.rs`.

use std::num::NonZeroU32;
use std::path::Path;

use serde_json::json;

pub(crate) const PASSWORD_OK: &str = "correct horse battery";

/// The permanent token from the `web-token` file.
pub(crate) fn file_token(dir: &Path) -> String {
    let text = std::fs::read_to_string(dir.join("web-token")).expect("web-token");
    text.trim().to_string()
}

/// The `web-password` file in the on-disk format (0600); the hash is computed by `ring` right here.
pub(crate) fn write_password(dir: &Path, password: &str, iterations: u32) {
    let salt = [7u8; 16];
    let mut hash = [0u8; 32];
    let n = NonZeroU32::new(iterations).expect("iterations");
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        n,
        &salt,
        password.as_bytes(),
        &mut hash,
    );
    let b64 = |b: &[u8]| data_encoding::BASE64.encode(b);
    let doc = json!({
        "version": 1, "alg": "pbkdf2-sha256", "iterations": iterations,
        "salt": b64(&salt), "hash": b64(&hash), "set_at": 1_700_000_000u64,
    });
    let path = dir.join("web-password");
    std::fs::write(&path, format!("{doc}\n")).expect("writing web-password");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("permissions");
    }
}
