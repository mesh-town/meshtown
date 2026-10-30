//! Tests of the default subcommand substitution (see `cli.rs`): starting with no arguments behaves
//! like `tray` (a build with the tray) or `run` (without it); explicit subcommands and flags don't change.

use std::ffi::OsString;

use clap::error::ErrorKind;
use clap::FromArgMatches;

use crate::cli::args::{after_help_text, with_default_subcommand, Cli, CliCommand};
use crate::cli::test_args::to_os;

/// The tray flag for tests that only check that the default does not interfere: the build's own.
const HAS_TRAY: bool = cfg!(feature = "gui");

#[test]
fn no_args_is_help_without_tray() {
    let err = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&["meshtown"]), false))
        .expect_err("no subcommand and no tray - the help");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

/// The build without the tray: no arguments print the root help to stdout (what the binary does with code 0).
#[cfg(not(feature = "gui"))]
#[test]
fn no_args_prints_root_help_in_this_build() {
    let err = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&["meshtown"]), HAS_TRAY))
        .expect_err("no tray in this build - the help");
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    assert!(err.to_string().contains("Global options:"), "{err}");
}

#[cfg(feature = "gui")]
#[test]
fn no_args_is_tray_with_tray() {
    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&["meshtown"]), true))
        .expect("no subcommand and with tray — tray");
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::Tray(_)
    ));
}

#[test]
fn explicit_subcommand_kept() {
    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(
            to_os(&["meshtown", "keygen"]),
            HAS_TRAY,
        ))
        .expect("keygen parses");
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::Keygen { .. }
    ));

    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(
            to_os(&["meshtown", "config-path"]),
            HAS_TRAY,
        ))
        .expect("config-path parses");
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::ConfigPath
    ));

    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(
            to_os(&["meshtown", "run", "--web-port", "7000"]),
            HAS_TRAY,
        ))
        .expect("run --web-port 7000 parses");
    let cli = Cli::from_arg_matches(&m).expect("parse");
    assert_eq!(cli.globals.web_port, Some(7000));
    assert!(matches!(cli.command, CliCommand::Run(_)));
}

#[test]
fn run_port_flag_is_an_unknown_argument() {
    let err = Cli::command()
        .try_get_matches_from(with_default_subcommand(
            to_os(&["meshtown", "run", "--port", "7000"]),
            HAS_TRAY,
        ))
        .expect_err("--port is not a flag of run");
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn flags_without_subcommand_still_error() {
    let err = Cli::command()
        .try_get_matches_from(with_default_subcommand(
            to_os(&["meshtown", "--port", "1"]),
            HAS_TRAY,
        ))
        .expect_err("a flag without a subcommand must not silently become run");
    assert_ne!(err.kind(), ErrorKind::DisplayHelp);
}

#[test]
fn empty_argv_untouched() {
    assert!(with_default_subcommand(Vec::<OsString>::new(), HAS_TRAY).is_empty());
}

#[test]
fn password_reset_parses() {
    let m = Cli::command()
        .try_get_matches_from(to_os(&[
            "meshtown",
            "password",
            "reset",
            "--web-port",
            "7001",
        ]))
        .expect("password reset parses");
    let cli = Cli::from_arg_matches(&m).expect("parse");
    assert_eq!(cli.globals.web_port, Some(7001));
    assert!(matches!(cli.command, CliCommand::Password { .. }));
}

#[test]
fn password_needs_the_reset_word() {
    assert!(Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "password", "forget"]))
        .is_err());
}

mod default_subcommand {
    use super::*;

    #[test]
    fn default_subcommand_without_tray_is_help() {
        // given
        let args = to_os(&["meshtown"]);

        // when
        let got = with_default_subcommand(args, false);

        // then
        assert_eq!(got.get(1), Some(&OsString::from("--help")));
    }

    #[test]
    fn default_subcommand_with_tray_is_tray() {
        let args = to_os(&["meshtown"]);

        let got = with_default_subcommand(args, true);

        assert_eq!(got.get(1), Some(&OsString::from("tray")));
    }

    #[test]
    fn explicit_subcommand_is_kept_with_tray() {
        let args = to_os(&["meshtown", "keygen"]);

        let got = with_default_subcommand(args.clone(), true);

        assert_eq!(got, args, "an explicit subcommand gets no insertion");
    }
}

mod after_help {
    use super::*;

    #[test]
    fn after_help_names_the_icon_with_tray() {
        assert!(after_help_text(true).contains("icon starts next to the clock"));
    }

    #[test]
    fn after_help_names_mesh_town_without_tray() {
        assert!(after_help_text(false).contains("meshtown run"));
    }
}

#[test]
fn open_parses_plain_and_with_flags() {
    let m = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "open"]))
        .expect("open parses");
    let cli = Cli::from_arg_matches(&m).expect("parse");
    match cli.command {
        CliCommand::Open { print, .. } => assert!(!print && cli.globals.web_port.is_none()),
        _ => panic!("expected open"),
    }
    let m = Cli::command()
        .try_get_matches_from(to_os(&[
            "meshtown",
            "open",
            "--print",
            "--web-port",
            "7778",
        ]))
        .expect("open --print parses");
    let cli = Cli::from_arg_matches(&m).expect("parse");
    match cli.command {
        CliCommand::Open { print, .. } => assert!(print && cli.globals.web_port == Some(7778)),
        _ => panic!("expected open"),
    }
}

#[test]
fn status_join_invite_leave_stop_parse() {
    use crate::cli::test_args::parse_ok;
    let (cli, _) = parse_ok(&["status", "--wait-online", "--timeout", "5"]);
    assert!(matches!(cli.command, CliCommand::Status(_)));
    let (cli, _) = parse_ok(&["join", "abc"]);
    assert!(matches!(cli.command, CliCommand::Join(_)));
    let (cli, _) = parse_ok(&["invite", "--for", "Ann"]);
    assert!(matches!(cli.command, CliCommand::Invite(_)));
    let (cli, _) = parse_ok(&["leave", "--yes"]);
    assert!(cli.globals.yes && matches!(cli.command, CliCommand::Leave(_)));
    let (cli, _) = parse_ok(&["stop"]);
    assert!(matches!(cli.command, CliCommand::Stop(_)));
}

#[test]
fn timeout_without_wait_online_is_refused() {
    let err = crate::cli::test_args::parse_err(&["status", "--timeout", "5"]);
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}
