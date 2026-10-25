//! Tests of the command line error texts (`cli_errors.rs`): the error report and the handling of a wrong command.

use super::*;
use crate::cli::args::Cli;
use crate::proxy::BindError;

fn parse_err(args: &[&str]) -> clap::Error {
    let mut all = vec!["meshtown"];
    all.extend_from_slice(args);
    Cli::command()
        .try_get_matches_from(all)
        .expect_err("parsing must refuse")
}

fn busy(port: u16) -> anyhow::Error {
    anyhow::Error::new(BindError {
        port,
        io_error_kind: std::io::ErrorKind::AddrInUse,
        os_message: "Address already in use (os error 48)".into(),
    })
}

#[test]
fn report_joins_causes() {
    let e = anyhow::anyhow!("a").context("b");
    assert_eq!(error_report(&e), "Error: b: a");
}

#[test]
fn report_keeps_own_prefix() {
    let e = anyhow::anyhow!("x").context("Error: already marked");
    let text = error_report(&e);
    assert_eq!(text, "Error: already marked: x");
}

#[test]
fn report_adds_prefix_to_text_starting_with_error_word() {
    let e = anyhow::anyhow!("x").context("Errors in the file");
    assert_eq!(error_report(&e), "Error: Errors in the file: x");
}

#[test]
fn report_adds_proxy_advice() {
    let text = error_report(&busy(1080));
    assert_eq!(
        text,
        "Error: port 1080 for the browser proxy is in use by another program (Address already in use (os error 48)).\n\
         Close it or start Mesh Town on another port: meshtown run --socks-port 1081"
    );
}

#[test]
fn report_adds_advice_under_context() {
    let text = error_report(&busy(1080).context("starting the node"));
    assert!(text.starts_with("Error: starting the node: port 1080"));
    assert!(text.ends_with("--socks-port 1081"));
    assert_eq!(text.lines().count(), 2);
}

#[test]
fn classify_unknown_command() {
    assert_eq!(
        classify(&parse_err(&["bogus"])),
        ParseFail::UnknownCommand("bogus".into())
    );
}

#[test]
fn classify_help_word() {
    assert_eq!(classify(&parse_err(&["help"])), ParseFail::HelpWord);
}

// `--port 1` without a subcommand is an unknown
// top-level option — `BadFlag` (see `classify_kind::unknown_flag::before_command_is_bad_flag`).
#[test]
fn classify_other_is_clap() {
    assert_eq!(classify(&parse_err(&["--help"])), ParseFail::LeaveToClap);
    assert_eq!(classify(&parse_err(&["-V"])), ParseFail::LeaveToClap);
}

#[test]
fn unknown_text_lists_commands() {
    let names = command_names(&Cli::command());
    #[cfg(feature = "gui")]
    assert_eq!(
        names,
        [
            "tray",
            "run",
            "keygen",
            "config-path",
            "password",
            "access",
            "ifaces"
        ]
    );
    #[cfg(not(feature = "gui"))]
    assert_eq!(
        names,
        [
            "run",
            "keygen",
            "config-path",
            "password",
            "access",
            "ifaces"
        ]
    );
    assert_eq!(
        unknown_command_text("bogus", &names),
        format!(
            "Error: unknown command \"bogus\".\nCommands: {}. Help: meshtown --help",
            names.join(", ")
        )
    );
}

// ---- Own texts for a wrong option and a wrong value ----

/// The kind of parse refusal: `classify` tells a wrong option and a wrong value from the rest.
mod classify_kind {
    use super::*;

    mod unknown_flag {
        use super::*;

        #[test]
        fn in_run_is_bad_flag() {
            assert_eq!(
                classify(&parse_err(&["run", "--bogus-flag"])),
                ParseFail::BadFlag
            );
        }

        #[test]
        fn in_keygen_is_bad_flag() {
            assert_eq!(
                classify(&parse_err(&["keygen", "--bogus"])),
                ParseFail::BadFlag
            );
        }

        #[test]
        fn short_is_bad_flag() {
            assert_eq!(classify(&parse_err(&["run", "-z"])), ParseFail::BadFlag);
        }

        #[test]
        fn before_command_is_bad_flag() {
            assert_eq!(classify(&parse_err(&["--port", "1"])), ParseFail::BadFlag);
        }
    }

    mod bad_value {
        use super::*;

        #[test]
        fn number_out_of_range_is_bad_value() {
            let err = parse_err(&["run", "--socks-port", "80"]);
            assert_eq!(classify(&err), ParseFail::BadValue);
        }

        #[test]
        fn not_a_number_is_bad_value() {
            let err = parse_err(&["run", "--socks-port", "abc"]);
            assert_eq!(classify(&err), ParseFail::BadValue);
        }

        #[test]
        fn unparsable_address_is_bad_value() {
            let err = parse_err(&["run", "--check-doh", "notanaddr"]);
            assert_eq!(classify(&err), ParseFail::BadValue);
        }

        // A flag without a value (`run --socks-port`) is `ErrorKind::InvalidValue` in clap with an empty value
        // and by contract also `BadValue`.
        #[test]
        fn missing_value_is_bad_value() {
            let err = parse_err(&["run", "--socks-port"]);
            assert_eq!(classify(&err), ParseFail::BadValue);
        }
    }

    mod other_errors {
        use super::*;

        #[test]
        fn subcommand_help_stays_clap() {
            assert_eq!(
                classify(&parse_err(&["run", "--help"])),
                ParseFail::LeaveToClap
            );
        }

        #[test]
        fn conflicting_flags_stay_clap() {
            let err = parse_err(&["run", "--no-config", "--save-config"]);
            assert_eq!(classify(&err), ParseFail::LeaveToClap);
        }

        #[test]
        fn no_command_stays_clap() {
            assert_eq!(classify(&parse_err(&[])), ParseFail::LeaveToClap);
        }
    }
}

/// Which of the typed words is the subcommand (for the tail "All options of the command: …").
mod typed_command_word {
    use super::*;

    #[test]
    fn finds_run() {
        let args = ["meshtown", "run", "--socks-port", "80"];
        assert_eq!(typed(&args), Some("run".to_string()));
    }

    #[test]
    fn finds_keygen() {
        assert_eq!(
            typed(&["meshtown", "keygen", "--raw"]),
            Some("keygen".to_string())
        );
    }

    #[test]
    fn finds_config_path() {
        assert_eq!(
            typed(&["meshtown", "config-path"]),
            Some("config-path".to_string())
        );
    }

    #[test]
    fn none_without_command() {
        assert_eq!(typed(&["meshtown"]), None);
        assert_eq!(typed(&["meshtown", "--help"]), None);
    }

    #[test]
    fn none_for_unknown_word() {
        assert_eq!(typed(&["meshtown", "bogus"]), None);
    }

    #[test]
    fn skips_words_that_are_not_commands() {
        let args = ["meshtown", "--bogus", "7", "run"];
        assert_eq!(typed(&args), Some("run".to_string()));
    }

    #[test]
    fn takes_the_first_of_several() {
        let args = ["meshtown", "keygen", "run"];
        assert_eq!(typed(&args), Some("keygen".to_string()));
    }

    #[test]
    fn program_name_is_not_a_command() {
        assert_eq!(typed(&["run"]), None);
        assert_eq!(typed(&["run", "keygen"]), Some("keygen".to_string()));
    }

    #[test]
    fn hidden_command_is_not_found() {
        let visible = strings(&["run", "config-path"]);
        assert_eq!(typed_command(&os(&["meshtown", "keygen"]), &visible), None);
    }

    #[test]
    fn real_command_list_knows_every_visible_command() {
        let names = command_names(&Cli::command());
        for name in &names {
            let found = typed_command(&os(&["meshtown", name.as_str()]), &names);
            assert_eq!(found.as_ref(), Some(name), "{name}");
        }
    }
}

// ---- helpers of the new tests ----

fn os(args: &[&str]) -> Vec<std::ffi::OsString> {
    args.iter().map(std::ffi::OsString::from).collect()
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// The subcommand `typed_command` found among the three visible ones.
fn typed(args: &[&str]) -> Option<String> {
    let visible = strings(&[
        "run",
        "keygen",
        "config-path",
        "password",
        "access",
        "ifaces",
    ]);
    typed_command(&os(args), &visible)
}
