use super::super::parse_net_text;
use super::*;
use crate::config::config_file::test_support::{all_changed, test_secret};
use crate::config::config_file::{NetSettings, SecretRead};

const NOW: u64 = 1_800_000_000;
const CREATED: u64 = 1_792_749_600;

mod all_changed_net {
    use super::*;

    #[test]
    fn has_no_process_sections() {
        // given: net settings where every field differs from the default
        let net = all_changed().net;
        // when: rendered
        let sections = config_sections(&render(&net));
        // then: none of the process-only sections is written
        for process_only in ["socks", "web", "log", "autostart", "full_tunnel"] {
            assert!(
                !sections.contains(&process_only.to_string()),
                "section {process_only} is in the slot file: {sections:?}"
            );
        }
    }

    #[test]
    fn node_section_holds_only_the_name() {
        // given: net settings where every field differs from the default
        let net = all_changed().net;
        // when: rendered
        let node = config_value(&render(&net), "node");
        // then: the section has the name and no memory limits
        let keys: Vec<String> = node
            .as_object()
            .expect("node is an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, ["name"]);
    }

    #[test]
    fn matches_the_golden_file_byte_for_byte() {
        // given: net settings where every field differs from the default
        let net = all_changed().net;
        // when: rendered with a fixed revision, time and creation time
        let text = render(&net);
        // then: the text is exactly the expected file
        assert_eq!(text, ALL_CHANGED_TEXT);
    }

    #[test]
    fn survives_render_then_parse() {
        // given: net settings where every field differs from the default
        let net = all_changed().net;
        // when: rendered and parsed back
        let p = parse_net_text(&render(&net)).expect("the rendered text parses");
        // then: the same settings, envelope values and no complaints
        assert_eq!(p.net, net);
        assert_eq!(p.rev, 1);
        assert_eq!(p.created_unix, CREATED);
        match &p.secret {
            SecretRead::Valid(k) => assert_eq!(k.to_bytes(), test_secret().to_bytes()),
            other => panic!("expected Valid, got {other:?}"),
        }
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        assert!(p.unknown_paths.is_empty(), "{:?}", p.unknown_paths);
        assert!(p.process_paths.is_empty(), "{:?}", p.process_paths);
    }
}

mod default_net {
    use super::*;

    #[test]
    fn matches_the_golden_file_byte_for_byte() {
        // given: default net settings, with no node name and no network key
        let net = NetSettings::default();
        // when: rendered
        let text = render(&net);
        // then: sections without keys stay as `{}`, the `net` section is dropped
        assert_eq!(text, DEFAULTS_TEXT);
    }
}

mod created_unix {
    use super::*;

    #[test]
    fn is_written_as_given() {
        // given: a creation time different from the save time
        let net = NetSettings::default();
        // when: rendered
        let value = root_value(&render(&net), "created_unix");
        // then: the number is kept
        assert_eq!(value.as_u64(), Some(CREATED));
    }
}

fn render(net: &NetSettings) -> String {
    render_net_file(1, NOW, CREATED, &test_secret(), net)
}

fn root_value(text: &str, key: &str) -> serde_json::Value {
    let root: serde_json::Value = serde_json::from_str(text).expect("the output is JSON");
    root[key].clone()
}

fn config_value(text: &str, section: &str) -> serde_json::Value {
    root_value(text, "config")[section].clone()
}

fn config_sections(text: &str) -> Vec<String> {
    let config = root_value(text, "config");
    let map = config.as_object().expect("config is an object");
    map.keys().cloned().collect()
}

// ---- expected literals ----

const DEFAULTS_TEXT: &str = r#"{
  "rev": 1,
  "saved_at": "2027-01-15T08:00:00Z",
  "created_unix": 1792749600,
  "identity": {
    "secret": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
  },
  "config": {
    "version": 1,
    "node": {},
    "network": {},
    "transport": {
      "mode": "direct_and_relay"
    },
    "exit": {
      "enabled": true
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
    }
  }
}
"#;

const ALL_CHANGED_TEXT: &str = r#"{
  "rev": 1,
  "saved_at": "2027-01-15T08:00:00Z",
  "created_unix": 1792749600,
  "identity": {
    "secret": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
  },
  "config": {
    "version": 1,
    "node": {
      "name": "台所 \"1\""
    },
    "network": {
      "key": "鍵🔑--test-key-not-real"
    },
    "transport": {
      "mode": "relay_only"
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
    }
  }
}
"#;
