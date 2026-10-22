//! Tests for retired config keys (`retired.rs`): parsing an old file, warning lines,
//! the `--port` flag, writing a file without the retired keys. Included from `retired.rs`:
//! `#[cfg(test)] #[path = "retired_tests.rs"] mod tests;`.

use super::*;
use crate::config::config_file::parse::parse_text;
use crate::config::config_file::test_support::{sample_settings, test_secret};
use crate::config::config_file::{ParsedConfigFile, TransportMode};

/// The `transport` section of a 0.1.37 file with all eight keys and a value for each, plus `socks`.
const ALL_EIGHT_WITH_VALUES: &str = r#""transport":{"mode":"relay_only","udp_port":70000,
    "congestion_control":"bbr","mtu_upper_bound":1199,"stream_receive_window":1,"receive_window":1,
    "max_concurrent_bidi_streams":0,"loss_packet_threshold":2,"loss_time_threshold":4.5},
    "socks":{"port":2000}"#;

mod list_of_retired_paths {
    use super::*;

    #[test]
    fn is_exactly_the_eight_iroh_keys_of_transport() {
        assert_eq!(
            RETIRED_PATHS,
            [
                "transport.udp_port",
                "transport.congestion_control",
                "transport.mtu_upper_bound",
                "transport.stream_receive_window",
                "transport.receive_window",
                "transport.max_concurrent_bidi_streams",
                "transport.loss_packet_threshold",
                "transport.loss_time_threshold",
            ]
        );
    }

    #[test]
    fn is_retired_only_in_transport_section() {
        assert!(
            is_retired("transport", "udp_port"),
            "transport.udp_port is retired"
        );
        assert!(
            !is_retired("socks", "udp_port"),
            "socks.udp_port is not retired"
        );
        assert!(!is_retired("transport", "mode"), "transport.mode is live");
        assert!(
            !is_retired("transport", "zzz"),
            "an unknown key is not retired"
        );
    }
}

mod old_file_with_all_eight_keys {
    use super::*;

    #[test]
    fn reads_without_errors_and_lists_keys_in_retired() {
        // given: a 0.1.37 file where all eight keys carry values (including out-of-range ones)
        // when: parse_text
        let p = parse_with(ALL_EIGHT_WITH_VALUES);
        // then: no errors and no unknown fields, the keys are in retired in list order, the rest was read
        assert_eq!(
            p.field_errors,
            vec![],
            "values of retired keys are not checked"
        );
        assert!(
            p.unknown_paths.is_empty(),
            "retired keys are not unknown: {:?}",
            p.unknown_paths
        );
        assert_eq!(p.retired_paths, all_retired_paths());
        assert_eq!(p.settings.net.transport_mode, TransportMode::RelayOnly);
        assert_eq!(p.settings.process.socks_port, 2000);
    }

    #[test]
    fn gets_one_warning_per_key_in_list_order() {
        // given: retired from all eight keys
        let retired = all_retired_paths();
        // when: retired_key_warnings
        let lines = retired_key_warnings(&retired);
        // then: eight lines, the i-th names the i-th path and the version since which the key is retired
        assert_eq!(lines.len(), 8);
        for (line, path) in lines.iter().zip(RETIRED_PATHS) {
            assert!(
                line.contains(&format!("config.{path}")),
                "path {path} is missing: {line}"
            );
            assert!(
                line.contains(RETIRED_SINCE),
                "version {RETIRED_SINCE} is missing: {line}"
            );
        }
    }

    #[test]
    fn null_values_are_skipped_silently() {
        // given: all eight keys with a null value (as the 0.1.37 file wrote them by default)
        // when: parse_text
        let p = parse_with(&transport_with_all_values("null"));
        // then: nothing to warn about, no unknown fields
        assert!(p.retired_paths.is_empty(), "retired: {:?}", p.retired_paths);
        assert!(retired_key_warnings(&p.retired_paths).is_empty());
        assert!(
            p.unknown_paths.is_empty(),
            "unknown fields: {:?}",
            p.unknown_paths
        );
    }

    #[test]
    fn mixed_null_and_value_lists_only_valued_keys() {
        // given: udp_port with a value, congestion_control null, loss_time_threshold with a value
        let p = parse_with(
            r#""transport":{"udp_port":4000,"congestion_control":null,"loss_time_threshold":2.5}"#,
        );
        // then: the list holds only keys with a value, in list order
        assert_eq!(
            p.retired_paths,
            vec![
                "config.transport.udp_port",
                "config.transport.loss_time_threshold"
            ]
        );
    }
}

mod retired_key_next_to_unknown_key {
    use super::*;

    #[test]
    fn ordinary_goes_to_unknown_and_retired_does_not() {
        // given: transport with a retired udp_port and an unknown zzz
        let p = parse_with(r#""transport":{"mode":"direct_and_relay","udp_port":1,"zzz":1}"#);
        // then: unknown holds only zzz, retired holds only udp_port
        assert_eq!(p.unknown_paths, vec!["config.transport.zzz"]);
        assert_eq!(p.retired_paths, vec!["config.transport.udp_port"]);
    }

    #[test]
    fn same_name_in_foreign_section_stays_unknown() {
        // given: a udp_port key in the socks section
        let p = parse_with(r#""socks":{"udp_port":1}"#);
        // then: it is unknown, nothing is retired
        assert_eq!(p.unknown_paths, vec!["config.socks.udp_port"]);
        assert!(p.retired_paths.is_empty(), "retired: {:?}", p.retired_paths);
    }
}

mod nothing_to_report {
    use super::*;

    #[test]
    fn no_retired_keys_when_transport_is_absent_or_not_an_object() {
        for sections in ["", r#""transport":5"#, r#""transport":null"#] {
            let p = parse_with(sections);
            assert!(
                p.retired_paths.is_empty(),
                "{sections:?}: retired {:?}",
                p.retired_paths
            );
        }
    }

    #[test]
    fn no_warning_lines_without_retired_keys() {
        assert!(retired_key_warnings(&[]).is_empty());
    }

    #[test]
    fn no_flag_warning_without_port_flag() {
        assert_eq!(port_flag_warning(None), None);
    }
}

mod port_flag {
    use super::*;

    #[test]
    fn warns_with_flag_name_and_version() {
        // given: the --port flag was given
        // when: port_flag_warning
        let line = port_flag_warning(Some(4000)).expect("the flag was given, so there is a line");
        // then: it holds the flag and the version since which it has no effect
        assert!(line.contains("--port"), "flag name is missing: {line}");
        assert!(
            line.contains(RETIRED_SINCE),
            "version {RETIRED_SINCE} is missing: {line}"
        );
    }
}

mod written_file {
    use super::*;

    #[test]
    fn carries_none_of_the_eight_keys() {
        // given: settings where no field has its default value
        // when: render_config_file
        let text = crate::config::config_file::render_config_file(
            1,
            0,
            &test_secret(),
            &sample_settings(),
        );
        // then: no retired key appears in quotes in the text
        for path in RETIRED_PATHS {
            let key = path
                .strip_prefix("transport.")
                .expect("path is in the transport section");
            assert!(
                !text.contains(&format!("\"{key}\"")),
                "retired key {key} is in the file"
            );
        }
    }
}

// ---- setup and helpers ----

/// Parses a file: `config` of version 1 and the listed sections (JSON text without outer braces).
fn parse_with(sections: &str) -> ParsedConfigFile {
    let sep = if sections.is_empty() { "" } else { "," };
    let text = format!(r#"{{"config":{{"version":1{sep}{sections}}}}}"#);
    parse_text(&text).expect("the file parses")
}

/// Returns `config.<path>` for each of the eight paths, in list order.
fn all_retired_paths() -> Vec<String> {
    RETIRED_PATHS
        .iter()
        .map(|p| format!("config.{p}"))
        .collect()
}

/// Returns the `transport` section with eight retired keys, each with the value `value` (JSON text).
fn transport_with_all_values(value: &str) -> String {
    let pairs: Vec<String> = RETIRED_PATHS
        .iter()
        .map(|p| {
            p.strip_prefix("transport.")
                .expect("path is in the transport section")
        })
        .map(|key| format!("\"{key}\":{value}"))
        .collect();
    format!(r#""transport":{{{}}}"#, pairs.join(","))
}
