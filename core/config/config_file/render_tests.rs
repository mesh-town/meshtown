use super::*;
use crate::config::config_file::settings::Settings;
use crate::config::config_file::test_support::test_secret;
use crate::config::config_file::{NetSettings, ProcessSettings, RelayMode};
use crate::transport::net::{NetChoice, NodeSecret, PeerId};

#[test]
fn name_absent_not_written() {
    let text = render_config_file(1, 0, &test_secret(), &Settings::default());
    assert!(
        !text.contains("\"name\""),
        "name was written for an unnamed entry: {text}"
    );
    assert!(
        text.contains("\"memory_warn_mib\": 400") && text.contains("\"memory_exit_mib\": 900"),
        "the node section has no memory guard thresholds: {text}"
    );
}

#[test]
fn route_exits_render_as_strings_in_order() {
    let a = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let b = NodeSecret::from_bytes(&[2u8; 32]).peer_id();
    let settings = Settings {
        net: NetSettings {
            route_exits: vec![a, b],
            ..NetSettings::default()
        },
        ..Settings::default()
    };

    let text = render_config_file(1, 0, &test_secret(), &settings);

    let doc: serde_json::Value = serde_json::from_str(&text).expect("render produces valid JSON");
    assert_eq!(
        doc["config"]["route"]["exits"],
        serde_json::json!([a.to_string(), b.to_string()])
    );
}

#[test]
fn route_exits_roundtrip_keeps_order() {
    let a = NodeSecret::from_bytes(&[3u8; 32]).peer_id();
    let b = NodeSecret::from_bytes(&[4u8; 32]).peer_id();
    let settings = Settings {
        net: NetSettings {
            route_exits: vec![b, a],
            ..NetSettings::default()
        },
        ..Settings::default()
    };

    let text = render_config_file(1, 0, &test_secret(), &settings);
    let parsed =
        crate::config::config_file::parse_text(&text).expect("parsing render's own output");

    assert_eq!(parsed.settings.net.route_exits, vec![b, a]);
}

#[test]
fn ports_roundtrip_and_for_always_written() {
    use crate::ports_sharing::types::{Audience, Opened, Proto, Published};
    let a = NodeSecret::from_bytes(&[3u8; 32]).peer_id();
    let settings = Settings {
        net: NetSettings {
            ports_published: vec![
                Published {
                    proto: Proto::Tcp,
                    port: 8080,
                    name: "web".to_string(),
                    audience: Audience::All,
                },
                Published {
                    proto: Proto::Tcp,
                    port: 22,
                    name: String::new(),
                    audience: Audience::Nodes(vec![a]),
                },
            ],
            ports_opened: vec![Opened {
                proto: Proto::Tcp,
                node: a,
                port: 8080,
                local: 9090,
            }],
            ..NetSettings::default()
        },
        ..Settings::default()
    };

    let text = render_config_file(1, 0, &test_secret(), &settings);
    let doc: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(doc["config"]["ports"]["published"][0]["for"], "all");
    assert_eq!(
        doc["config"]["ports"]["published"][1]["for"],
        serde_json::json!([a.to_string()])
    );
    let parsed =
        crate::config::config_file::parse_text(&text).expect("parsing render's own output");

    assert!(parsed.field_errors.is_empty(), "{:?}", parsed.field_errors);
    assert_eq!(
        parsed.settings.net.ports_published,
        settings.net.ports_published
    );
    assert_eq!(parsed.settings.net.ports_opened, settings.net.ports_opened);
}

#[test]
fn access_rendered() {
    use crate::config::config_file::settings::ListenAccess;
    let settings = Settings {
        process: ProcessSettings {
            web_access: ListenAccess::Localhost,
            ..ProcessSettings::default()
        },
        ..Settings::default()
    };
    let text = render_config_file(1, 0, &test_secret(), &settings);
    let doc: serde_json::Value = serde_json::from_str(&text).expect("render produces valid JSON");
    assert_eq!(doc["config"]["web"]["access"], "localhost");
    let text = render_config_file(1, 0, &test_secret(), &Settings::default());
    let doc: serde_json::Value = serde_json::from_str(&text).expect("render produces valid JSON");
    assert_eq!(doc["config"]["web"]["access"], "localnet");
}

/// Returns the JSON that `render_config_file` writes to the settings file for these settings.
fn rendered(settings: &Settings) -> serde_json::Value {
    let text = render_config_file(1, 0, &test_secret(), settings);
    serde_json::from_str(&text).expect("render produces valid JSON")
}

fn golden_id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

#[test]
fn golden_render_writes_seed_as_base64_literal() {
    let text = render_config_file(1, 0, &test_secret(), &Settings::default());
    let doc: serde_json::Value = serde_json::from_str(&text).expect("render produces valid JSON");
    let expected = format!("{}Bwc=", "BwcH".repeat(10));
    assert_eq!(expected.len(), 44);
    assert_eq!(doc["identity"]["secret"], expected.as_str());
}

#[test]
fn golden_render_writes_exits_as_lowercase_hex() {
    const KEY1: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const KEY2: &str = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c";
    let a = golden_id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let b = golden_id_of("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
    // RFC 8032 §7.1 anchor: seed → public key
    assert_eq!(a.to_string(), KEY1);
    assert_eq!(b.to_string(), KEY2);
    let settings = Settings {
        net: NetSettings {
            route_exits: vec![a, b],
            ..NetSettings::default()
        },
        ..Settings::default()
    };
    let text = render_config_file(1, 0, &test_secret(), &settings);
    let doc: serde_json::Value = serde_json::from_str(&text).expect("render produces valid JSON");
    assert_eq!(
        doc["config"]["route"]["exits"],
        serde_json::json!([KEY1, KEY2])
    );
}

#[test]
fn net_omitted_when_default() {
    // given: default settings, the network choice was not changed
    let settings = Settings::default();
    // when: render_config_file
    let doc = rendered(&settings);
    // then: config has no net key — the default is not written
    let config = doc["config"].as_object().expect("config is an object");
    assert!(
        !config.contains_key("net"),
        "net was written at its default: {config:?}"
    );
}

#[test]
fn net_writes_only_changed_fields() {
    // given: of the three net fields only sync differs from the default
    let settings = settings_with_net(|net| net.sync = "mem".to_string());
    // when: render_config_file
    let doc = rendered(&settings);
    // then: net has exactly one field, default lists are not written
    assert_eq!(doc["config"]["net"], serde_json::json!({"sync": "mem"}));
}

#[test]
fn net_roundtrip() {
    // given: discovery and sync are not at their defaults
    let settings = settings_with_net(|net| {
        net.discovery = vec!["cache".to_string(), "invite".to_string()];
        net.sync = "mem".to_string();
    });
    // when: render_config_file, then parsing its own output
    let text = render_config_file(1, 0, &test_secret(), &settings);
    let parsed =
        crate::config::config_file::parse_text(&text).expect("parsing render's own output");
    // then: the network choice is the same, no field errors
    assert_eq!(parsed.settings.net, settings.net);
    assert!(parsed.field_errors.is_empty(), "{:?}", parsed.field_errors);
}

#[test]
fn explicit_default_dropped_on_write() {
    // given: a file where net is written explicitly with default values
    let parsed =
        crate::config::config_file::parse_text(&file_with_default_net()).expect("parsing the file");
    assert_eq!(
        parsed.settings.net.net_choice,
        Settings::default().net.net_choice,
        "test precondition: net was read as its default"
    );
    // when: parse -> render_config_file
    let doc = rendered(&parsed.settings);
    // then: net is absent on write
    let config = doc["config"].as_object().expect("config is an object");
    assert!(
        !config.contains_key("net"),
        "the explicit default stayed in the file: {config:?}"
    );
}

#[test]
fn relay_default_is_not_written() {
    // given: settings with the default relay mode
    let settings = Settings::default();
    // when: render_config_file
    let doc = rendered(&settings);
    // then: the file has no relay field, and no net section at all
    assert_eq!(settings.net.relay, RelayMode::Network);
    assert!(
        doc["config"]["net"]["relay"].is_null(),
        "the default relay mode was written: {}",
        doc["config"]
    );
}

#[test]
fn relay_off_is_written() {
    // given: relaying is switched off
    let settings = settings_with_relay(RelayMode::Off);
    // when: render_config_file
    let doc = rendered(&settings);
    // then: net holds exactly the relay field
    assert_eq!(doc["config"]["net"], serde_json::json!({"relay": "off"}));
}

#[test]
fn relay_off_round_trips_through_the_page_file() {
    // given: relaying is switched off
    let settings = settings_with_relay(RelayMode::Off);
    // when: render, then parse the output
    let text = render_config_file(1, 0, &test_secret(), &settings);
    let parsed =
        crate::config::config_file::parse_text(&text).expect("parsing render's own output");
    // then: the mode is read back, no complaints
    assert_eq!(parsed.settings.net.relay, RelayMode::Off);
    assert!(parsed.field_errors.is_empty(), "{:?}", parsed.field_errors);
}

fn settings_with_relay(relay: RelayMode) -> Settings {
    let mut settings = Settings::default();
    settings.net.relay = relay;
    settings
}

/// Returns default settings with the network choice modified by `change`.
fn settings_with_net(change: impl FnOnce(&mut NetChoice)) -> Settings {
    let mut settings = Settings::default();
    change(&mut settings.net.net_choice);
    settings
}

/// Returns a file text whose `net` section holds exactly the build's default values.
fn file_with_default_net() -> String {
    let d = Settings::default().net.net_choice;
    serde_json::json!({
        "config": {
            "version": 1,
            "net": {"transports": d.transports, "discovery": d.discovery, "sync": d.sync}
        }
    })
    .to_string()
}
