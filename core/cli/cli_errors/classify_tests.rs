//! Tests of the classification of refused command lines (`classify.rs`) and of the commands named in them.

use super::*;
use crate::cli::args::Cli;
use crate::cli::cli_errors::text::unknown_command_text;
use crate::cli::test_args::{parse_err, to_os as os};

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
            "tray", "run", "join", "status", "stop", "invite", "leave", "access", "config",
            "network", "password", "open"
        ]
    );
    #[cfg(not(feature = "gui"))]
    assert_eq!(
        names,
        [
            "run", "join", "status", "stop", "invite", "leave", "access", "config", "network",
            "password", "open"
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
        fn in_config_path_is_bad_flag() {
            assert_eq!(
                classify(&parse_err(&["config", "path", "--bogus"])),
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
            let err = parse_err(&["run", "--socks-access", "bogus"]);
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
        fn no_command_gets_error_form() {
            let err = parse_err(&["--config", "x"]);
            assert_eq!(err.kind(), clap::error::ErrorKind::MissingSubcommand);
            assert_eq!(classify(&err), ParseFail::Other);
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
    fn finds_network() {
        assert_eq!(
            typed(&["meshtown", "network", "--raw"]),
            Some("network".to_string())
        );
    }

    #[test]
    fn finds_config_path() {
        assert_eq!(typed(&["meshtown", "config"]), Some("config".to_string()));
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
        let args = ["meshtown", "network", "run"];
        assert_eq!(typed(&args), Some("network".to_string()));
    }

    #[test]
    fn program_name_is_not_a_command() {
        assert_eq!(typed(&["run"]), None);
        assert_eq!(typed(&["run", "network"]), Some("network".to_string()));
    }

    #[test]
    fn hidden_command_is_not_found() {
        let visible = strings(&["run", "config"]);
        assert_eq!(typed_command(&os(&["meshtown", "network"]), &visible), None);
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

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// The subcommand `typed_command` found among the three visible ones.
fn typed(args: &[&str]) -> Option<String> {
    let visible = strings(&["run", "network", "config", "password", "access"]);
    typed_command(&os(args), &visible)
}

mod error_forms {
    use super::*;

    #[test]
    fn missing_required_argument_is_capitalized_error() {
        let err = parse_err(&["access", "ifaces"]);
        assert_eq!(classify(&err), ParseFail::Other);
    }

    #[test]
    fn help_and_version_stay_clap() {
        for args in [&["--help"][..], &["-V"], &["status", "--help"]] {
            assert_eq!(
                classify(&parse_err(args)),
                ParseFail::LeaveToClap,
                "{args:?}"
            );
        }
    }
}

/// The help of `help`, `--help` and a start with no arguments (no tray) is one text.
#[test]
fn help_word_and_flag_and_empty_argv_print_the_same() {
    use crate::cli::args::{with_default_subcommand, Cli};
    use crate::cli::test_args::to_os;
    let render = |args: Vec<std::ffi::OsString>| {
        let err = Cli::command().try_get_matches_from(args).expect_err("help");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        err.render().to_string()
    };
    let commands: Vec<String> = Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    let word = help_args(&to_os(&["meshtown", "help"]), &commands).expect("help word");
    let flag = render(to_os(&["meshtown", "--help"]));
    assert_eq!(render(word), flag);
    assert_eq!(
        render(with_default_subcommand(to_os(&["meshtown"]), false)),
        flag
    );
}
