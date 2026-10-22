//! Renders the settings file text: `Serialize` structs in exact field order, without
//! unknown fields and without values computed by the node (`label`, `masked`, `available`, `reason`).

use super::fields::ConfigDoc;
use super::settings::Settings;
use super::time::iso_utc;
use crate::transport::net::NodeSecret;

#[derive(serde::Serialize)]
struct FileOut<'a> {
    rev: u64,
    saved_at: String,
    identity: IdentityOut,
    config: ConfigDoc<'a>,
}

/// Node secret: base64 of `NodeSecret::to_bytes()`.
#[derive(serde::Serialize)]
struct IdentityOut {
    secret: String,
}

/// Returns the file text: a document with revision `rev`, time `now_unix` and the node secret.
pub fn render_config_file(rev: u64, now_unix: u64, secret: &NodeSecret, s: &Settings) -> String {
    let doc = FileOut {
        rev,
        saved_at: iso_utc(now_unix),
        identity: IdentityOut {
            secret: data_encoding::BASE64.encode(&secret.to_bytes()),
        },
        config: ConfigDoc(s),
    };
    // The structs above hold only numbers, strings and string lists, so serialization cannot fail.
    let mut text =
        serde_json::to_string_pretty(&doc).expect("serializing the settings document never fails");
    text.push('\n');
    text
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "render_golden_tests.rs"]
mod golden_tests;
