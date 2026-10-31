//! Tests of the texts for a wrong option and a wrong value (`cli_errors.rs`:
//! `bad_flag_text` with the hint about a similar option, `bad_value_text`). Included from
//! `cli_errors` as `text_tests`.

use super::*;
use crate::cli::test_args::parse_err;

/// The text about a wrong option: `bad_flag_text`.
mod bad_flag_message {
    use super::*;

    #[test]
    fn names_flag_and_points_to_run_help() {
        assert_eq!(
            flag_text(&["run", "--bogus-flag"], Some("run")),
            "Error: unknown option \"--bogus-flag\". See all options: meshtown run --help"
        );
    }

    #[test]
    fn tail_follows_the_typed_command() {
        assert_eq!(
            flag_text(&["network", "--bogus"], Some("network")),
            "Error: unknown option \"--bogus\". See all options: meshtown network --help"
        );
    }

    #[test]
    fn tail_without_command_points_to_program_help() {
        assert_eq!(
            flag_text(&["--bogus"], None),
            "Error: unknown option \"--bogus\". See all options: meshtown --help"
        );
    }

    #[test]
    fn extra_word_without_dash_is_called_extra_word() {
        assert_eq!(
            flag_text(&["config-path", "extra"], Some("config-path")),
            "Error: unexpected argument \"extra\". See all options: meshtown config-path --help"
        );
    }

    #[test]
    fn typo_in_flag_gets_a_suggestion() {
        assert_eq!(
            flag_text(&["run", "--sock-port", "1080"], Some("run")),
            "Error: unknown option \"--sock-port\". Did you mean --socks-port? See all options: meshtown run --help"
        );
    }

    #[test]
    fn short_flag_is_named_as_typed() {
        assert_eq!(
            flag_text(&["run", "-z"], Some("run")),
            "Error: unknown option \"-z\". See all options: meshtown run --help"
        );
    }
}

/// The text about a wrong value: `bad_value_text`.
mod bad_value_message {
    use super::*;

    #[test]
    fn port_below_range_names_range() {
        assert_eq!(
            value_text(&["run", "--socks-port", "80"], Some("run")),
            "Error: invalid value \"80\" for --socks-port: use a number from 1024 to 65535. See all options: meshtown run --help"
        );
    }

    #[test]
    fn port_above_range_names_range() {
        assert_eq!(
            value_text(&["run", "--socks-port", "70000"], Some("run")),
            "Error: invalid value \"70000\" for --socks-port: use a number from 1024 to 65535. See all options: meshtown run --help"
        );
    }

    #[test]
    fn word_instead_of_number_asks_for_integer() {
        assert_eq!(
            value_text(&["run", "--socks-port", "abc"], Some("run")),
            "Error: invalid value \"abc\" for --socks-port: use a whole number. See all options: meshtown run --help"
        );
    }

    #[test]
    fn empty_string_instead_of_number_asks_for_integer() {
        assert_eq!(
            value_text(&["run", "--socks-port", ""], Some("run")),
            "Error: invalid value \"\" for --socks-port: use a whole number. See all options: meshtown run --help"
        );
    }

    #[test]
    fn custom_parser_reason_follows_colon() {
        assert_eq!(
            value_text(&["run", "--socks-access", "bogus"], Some("run")),
            "Error: invalid value \"bogus\" for --socks-access: allowed: localhost, localnet. See all options: meshtown run --help"
        );
    }

    #[test]
    fn tail_without_command_points_to_program_help() {
        assert_eq!(
            value_text(&["run", "--socks-port", "80"], None),
            "Error: invalid value \"80\" for --socks-port: use a number from 1024 to 65535. See all options: meshtown --help"
        );
    }

    #[cfg(feature = "gui")]
    #[test]
    fn tail_follows_the_typed_command() {
        assert_eq!(
            value_text(&["tray", "--web-port", "70000"], Some("tray")),
            "Error: invalid value \"70000\" for --web-port: use a number from 0 to 65535. See all options: meshtown tray --help"
        );
    }

    // A flag without a value is `InvalidValue` with an empty value: its own text.
    #[test]
    fn missing_value_reads_by_the_same_rules() {
        assert_eq!(
            value_text(&["run", "--socks-port"], Some("run")),
            "Error: --socks-port needs a value. See all options: meshtown run --help"
        );
    }
}

/// None of the new texts carries an English word from clap/serde.
mod no_english {
    use super::*;

    #[test]
    fn in_any_message() {
        let texts = [
            flag_text(&["run", "--bogus-flag"], Some("run")),
            flag_text(&["--bogus"], None),
            value_text(&["run", "--socks-port", "80"], Some("run")),
            value_text(&["run", "--socks-port", "abc"], Some("run")),
            value_text(&["run", "--socks-access", "bogus"], Some("run")),
        ];
        for text in &texts {
            for bad in [
                "error",
                "invalid value '",
                "Usage",
                "expected",
                "found",
                "digit",
                "parse",
            ] {
                assert!(!text.contains(bad), "\"{bad}\" in the text: {text}");
            }
            assert_eq!(text.lines().count(), 1, "one-line text: {text}");
        }
    }
}

// ---- helpers ----

/// The text about a wrong option for the typed command (without the program name in `args`).
fn flag_text(args: &[&str], command: Option<&str>) -> String {
    bad_flag_text(&parse_err(args), command)
}

/// The text about a wrong value for the typed command (without the program name in `args`).
fn value_text(args: &[&str], command: Option<&str>) -> String {
    bad_value_text(&parse_err(args), command)
}

mod other_form {
    use super::*;

    #[test]
    fn other_text_has_no_lowercase_error_prefix() {
        let err = parse_err(&["access", "ifaces"]);
        let text = other_text(&err, Some("access"));
        assert!(text.starts_with("Error: "), "{text}");
        assert!(!text.contains("error:"), "{text}");
        assert!(text.contains("See all options: "), "{text}");
    }
}
