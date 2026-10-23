//! The text of a slot file: the main file's envelope with `created_unix`, and the network rows only.

use super::super::fields::NetDoc;
use super::super::net_settings::NetSettings;
use super::super::settings::Settings;
use super::super::time::iso_utc;
use crate::transport::net::NodeSecret;

#[derive(serde::Serialize)]
struct FileOut<'a> {
    rev: u64,
    saved_at: String,
    created_unix: u64,
    identity: IdentityOut,
    config: NetDoc<'a>,
}

/// Node secret: base64 of `NodeSecret::to_bytes()`.
#[derive(serde::Serialize)]
struct IdentityOut {
    secret: String,
}

/// Returns the slot file text: revision `rev`, time `now_unix`, creation time and the slot's secret.
pub fn render_net_file(
    rev: u64,
    now_unix: u64,
    created_unix: u64,
    secret: &NodeSecret,
    net: &NetSettings,
) -> String {
    // The process block never reaches the output: `NetDoc` skips its rows.
    let settings = Settings {
        net: net.clone(),
        ..Settings::default()
    };
    let doc = FileOut {
        rev,
        saved_at: iso_utc(now_unix),
        created_unix,
        identity: IdentityOut {
            secret: data_encoding::BASE64.encode(&secret.to_bytes()),
        },
        config: NetDoc(&settings),
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
