//! Reference response of `GET /api/config` before the split of `Settings`: the part without the `file` block
//! (the on-disk path differs on every run). The file is not edited after the golden set was recorded.

use std::path::{Path, PathBuf};

use crate::config::config_file::{self, CliOverrides, ConfigSource, PathRule};

use super::ConfigState;

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
      "enabled": false
    },
    "socks": {
      "access": "localnet",
      "enabled": false,
      "port": 1090
    },
    "checks": {
      "enabled": false,
      "interval_sec": 600,
      "doh_server": "9.9.9.9:443",
      "provider_dns": "[::1]:53"
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
      "access": "localhost"
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
const BODY_TEXT: &str = r#"{
  "cli_overrides": [],
  "config": {
    "autostart": {
      "available": false,
      "enabled": false,
      "reason": "coming in a later version"
    },
    "checks": {
      "doh_server": "9.9.9.9:443",
      "enabled": false,
      "geo_markers": [],
      "interval_sec": 600,
      "provider_dns": "[::1]:53"
    },
    "exit": {
      "enabled": false,
      "max_streams": 0
    },
    "full_tunnel": {
      "available": false,
      "enabled": false,
      "reason": "coming in a later version"
    },
    "log": {
      "ring_lines": 500
    },
    "net": {
      "discovery": [
        "cache",
        "invite"
      ],
      "sync": "mem"
    },
    "network": {
      "label": "",
      "masked": "label--•••••••••••••••••••"
    },
    "node": {
      "memory_exit_mib": 800,
      "memory_warn_mib": 500,
      "name": "Kitchen \"1\""
    },
    "ports": {
      "opened": [
        {
          "local": 8081,
          "node": "0b513ad9b4924015ca0902ed079044d3ac5dbec2306f06948c10da8eb6e39f2d",
          "port": 80
        }
      ],
      "published": [
        {
          "for": "all",
          "name": "web",
          "port": 8080
        },
        {
          "for": [
            "66be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a"
          ],
          "name": "second",
          "port": 9090
        }
      ]
    },
    "probe": {
      "full_cycle": true,
      "interval_sec": 120
    },
    "route": {
      "exits": [
        "fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618",
        "43a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c"
      ]
    },
    "socks": {
      "access": "localnet",
      "enabled": false,
      "ifaces": [],
      "port": 1090
    },
    "version": 1,
    "web": {
      "access": "localhost",
      "enabled": false,
      "ifaces": [],
      "port": 8080
    }
  },
  "read_only": [
    "network.label",
    "network.masked",
    "web.enabled",
    "web.port",
    "autostart.available",
    "autostart.reason",
    "full_tunnel.available",
    "full_tunnel.reason"
  ],
  "restart_fields": [
    "socks.enabled",
    "socks.port",
    "socks.access",
    "socks.ifaces",
    "checks.doh_server",
    "checks.provider_dns"
  ]
}"#;

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-config-split-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn state_from_input(dir: &Path) -> ConfigState {
    let path = dir.join("config.json");
    std::fs::write(&path, INPUT).unwrap();
    let source = ConfigSource::File {
        path,
        may_create: false,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    ConfigState::from_run(&run)
}

fn config_part(state: &ConfigState) -> String {
    let body = state.config_get_body();
    let part = serde_json::json!({
        "config": body["config"],
        "read_only": body["read_only"],
        "restart_fields": body["restart_fields"],
        "cli_overrides": body["cli_overrides"],
    });
    serde_json::to_string_pretty(&part).unwrap()
}

#[test]
fn get_body_config_part_is_fixed() {
    // given: file INPUT; when: config_get_body; then: the part without `file` is verbatim equal to the reference
    let dir = temp_dir();
    let text = config_part(&state_from_input(&dir));
    cleanup(&dir);
    assert_eq!(text, BODY_TEXT);
}
