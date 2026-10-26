//! Reference split of `Settings` into blocks: only the text API — `parse_text`, `render_config_file`,
//! `{:?}`, the `FIELDS` table; no access to a `Settings` field. The file does not change after the split.

use crate::config::config_file::fields::FIELDS;
use crate::config::config_file::test_support::test_secret;
use crate::config::config_file::{parse_text, render_config_file, ParsedConfigFile, Settings};

const NOW: u64 = 1_700_000_000;
const REV: u64 = 7;

const DEFAULT_TEXT: &str = r#"{
  "rev": 7,
  "saved_at": "2023-11-14T22:13:20Z",
  "identity": {
    "secret": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
  },
  "config": {
    "version": 1,
    "node": {
      "memory_warn_mib": 400,
      "memory_exit_mib": 900
    },
    "network": {},
    "exit": {
      "enabled": true,
      "max_streams": 0
    },
    "socks": {
      "enabled": true,
      "port": 1080,
      "access": "localhost",
      "ifaces": []
    },
    "checks": {
      "enabled": true,
      "interval_sec": 900,
      "doh_server": "1.1.1.1:443",
      "provider_dns": null,
      "geo_markers": []
    },
    "probe": {
      "interval_sec": 300,
      "full_cycle": false
    },
    "route": {
      "exits": []
    },
    "ports": {
      "published": [],
      "opened": []
    },
    "web": {
      "enabled": true,
      "port": 7777,
      "access": "localnet",
      "ifaces": []
    },
    "log": {
      "ring_lines": 2000
    },
    "autostart": {
      "enabled": false
    },
    "full_tunnel": {
      "enabled": false
    }
  }
}
"#;
const DEBUG_DEFAULT: &str = r#"Settings { node.memory_warn_mib: 400, node.memory_exit_mib: 900, node.name: None, network.key: None, net.transports: ["hyper"], net.discovery: ["invite", "cache", "hyperdht"], net.sync: "mesh", exit.enabled: true, exit.max_streams: 0, socks.enabled: true, socks.port: 1080, socks.access: Localhost, socks.ifaces: Auto, proxy.net: NetSlotId("main"), checks.enabled: true, checks.interval_sec: 900, checks.doh_server: 1.1.1.1:443, checks.provider_dns: None, checks.geo_markers: [], probe.interval_sec: 300, probe.full_cycle: false, route.exits: [], ports.published: [], ports.opened: [], web.enabled: true, web.port: 7777, web.access: Localnet, web.ifaces: Auto, log.ring_lines: 2000, autostart.enabled: false, full_tunnel.enabled: false }"#;
const INPUT: &str = r#"{
  "rev": 7,
  "saved_at": "2023-11-14T22:13:20Z",
  "identity": {
    "secret": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
  },
  "config": {
    "version": 1,
    "node": {
      "memory_warn_mib": 500,
      "memory_exit_mib": 800,
      "name": "Kitchen \"1\""
    },
    "network": {
      "key": "label--sample-key-not-real"
    },
    "net": {
      "discovery": [
        "cache",
        "invite"
      ],
      "sync": "mem"
    },
    "exit": {
      "enabled": false,
      "max_streams": 40
    },
    "socks": {
      "enabled": false,
      "port": 1090,
      "access": "localnet",
      "ifaces": [
        "br-lan"
      ]
    },
    "proxy": {
      "net": "ab2c3d"
    },
    "checks": {
      "enabled": false,
      "interval_sec": 600,
      "doh_server": "9.9.9.9:443",
      "provider_dns": "[::1]:53",
      "geo_markers": [
        "ünavailable hère"
      ]
    },
    "probe": {
      "interval_sec": 120,
      "full_cycle": true
    },
    "route": {
      "exits": [
        "fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618",
        "43a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c"
      ]
    },
    "ports": {
      "published": [
        {
          "port": 8080,
          "name": "web",
          "for": "all"
        },
        {
          "port": 9090,
          "name": "second",
          "for": [
            "66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a"
          ]
        }
      ],
      "opened": [
        {
          "node": "0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d",
          "port": 80,
          "local": 8081
        }
      ]
    },
    "web": {
      "enabled": false,
      "port": 8080,
      "access": "localhost",
      "ifaces": [
        "*"
      ]
    },
    "log": {
      "ring_lines": 500
    },
    "autostart": {
      "enabled": false
    },
    "full_tunnel": {
      "enabled": false
    }
  }
}
"#;
const DEBUG_ALL: &str = r#"Settings { node.memory_warn_mib: 500, node.memory_exit_mib: 800, node.name: Some("Kitchen \"1\""), network.key: Some("<hidden>"), net.transports: ["hyper"], net.discovery: ["cache", "invite"], net.sync: "mem", exit.enabled: false, exit.max_streams: 40, socks.enabled: false, socks.port: 1090, socks.access: Localnet, socks.ifaces: Only(["br-lan"]), proxy.net: NetSlotId("ab2c3d"), checks.enabled: false, checks.interval_sec: 600, checks.doh_server: 9.9.9.9:443, checks.provider_dns: Some([::1]:53), checks.geo_markers: ["ünavailable hère"], probe.interval_sec: 120, probe.full_cycle: true, route.exits: [PeerId(fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618), PeerId(43a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c)], ports.published: [Published { port: 8080, proto: Tcp, name: "web", audience: All }, Published { port: 9090, proto: Tcp, name: "second", audience: Nodes([PeerId(66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a)]) }], ports.opened: [Opened { node: PeerId(0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d), port: 80, proto: Tcp, local: 8081 }], web.enabled: false, web.port: 8080, web.access: Localhost, web.ifaces: Any, log.ring_lines: 500, autostart.enabled: false, full_tunnel.enabled: false }"#;

fn render_of(s: &Settings) -> String {
    render_config_file(REV, NOW, &test_secret(), s)
}

fn parsed_input() -> ParsedConfigFile {
    let p = parse_text(INPUT).expect("parsing INPUT");
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "extra keys: {:?}",
        p.unknown_paths
    );
    p
}

#[test]
fn default_renders_fixed_text() {
    // given: defaults; when: render; then: the verbatim text
    assert_eq!(render_of(&Settings::default()), DEFAULT_TEXT);
}

#[test]
fn default_debug_is_fixed() {
    // given: defaults; when: Debug; then: the verbatim text
    assert_eq!(format!("{:?}", Settings::default()), DEBUG_DEFAULT);
}

#[test]
fn every_path_round_trips() {
    // given: a file with all paths; when: parse and render with the same REV, NOW; then: the same text
    let parsed = parsed_input();
    assert_eq!(render_of(&parsed.settings), INPUT);
}

#[test]
fn every_path_debug_is_fixed() {
    // given: the parsed INPUT; when: Debug; then: the verbatim text, the key hidden
    assert_eq!(format!("{:?}", parsed_input().settings), DEBUG_ALL);
}

#[test]
fn table_copy_moves_every_stored_path() {
    // given: defaults and the parsed INPUT; when: copy for every stored table row
    let parsed = parsed_input();
    let mut to = Settings::default();
    for row in FIELDS.iter().filter(|r| r.value.is_stored_in_settings()) {
        row.value.copy_value_between(&parsed.settings, &mut to);
    }
    // then: the copy's output equals INPUT
    assert_eq!(render_of(&to), INPUT);
}
