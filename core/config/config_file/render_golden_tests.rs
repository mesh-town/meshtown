//! Golden tests for `render_config_file` output on the original code: the whole file text.
//! The literals were captured by running the original code; `net` is written only as differences from the defaults.

use super::*;
use crate::config::config_file::settings::Settings;
use crate::config::config_file::test_support::{all_changed, test_secret};

const NOW: u64 = 1_800_000_000;

mod defaults {
    use super::*;

    #[test]
    fn render_defaults_golden() {
        // given: defaults; when: render_config_file; then: no `net` section, `network` is `{}`
        let text = render_config_file(1, NOW, &test_secret(), &Settings::default());
        assert_eq!(text, DEFAULTS_TEXT);
    }
}

mod all_changed_settings {
    use super::*;

    #[test]
    fn render_all_changed_golden() {
        // given: every field is non-default; when: render_config_file
        // then: `1.1` without a tail, `[::1]:53`, quotes in the name, `net` holds only differing keys
        let text = render_config_file(1, NOW, &test_secret(), &all_changed());
        assert_eq!(text, ALL_CHANGED_TEXT);
    }
}

// ---- expected literals ----

const DEFAULTS_TEXT: &str = r#"{
  "rev": 1,
  "saved_at": "2027-01-15T08:00:00Z",
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
      "enabled": true
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

const ALL_CHANGED_TEXT: &str = r#"{
  "rev": 1,
  "saved_at": "2027-01-15T08:00:00Z",
  "identity": {
    "secret": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
  },
  "config": {
    "version": 1,
    "node": {
      "memory_warn_mib": 500,
      "memory_exit_mib": 800,
      "name": "台所 \"1\""
    },
    "network": {
      "key": "鍵🔑--test-key-not-real"
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
          "name": "網",
          "for": "all"
        },
        {
          "port": 9090,
          "name": "二番",
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
      "enabled": true
    },
    "full_tunnel": {
      "enabled": true
    }
  }
}
"#;
