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
fn no_args_is_run_without_tray() {
    use crate::cli::args::is_flag_typed;

    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&["meshtown"]), false))
        .expect("no subcommand and no tray — run");
    let run_m = m
        .subcommand_matches("run")
        .expect("run subcommand substituted");
    assert!(
        !is_flag_typed(run_m, "socks_port"),
        "the default must not look typed"
    );
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::Run(_)
    ));
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
        CliCommand::ConfigPath { .. }
    ));

    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(
            to_os(&["meshtown", "run", "--web-port", "7000"]),
            HAS_TRAY,
        ))
        .expect("run --web-port 7000 parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Run(args) => assert_eq!(args.web_port, Some(7000)),
        _ => panic!("expected the run subcommand"),
    }
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
fn password_reset_and_access_parse() {
    let m = Cli::command()
        .try_get_matches_from(to_os(&[
            "meshtown",
            "password",
            "reset",
            "--web-port",
            "7001",
        ]))
        .expect("password reset parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Password { web_port, .. } => assert_eq!(web_port, Some(7001)),
        _ => panic!("expected password"),
    }
    let m = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "access", "localhost"]))
        .expect("access parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Access { mode, .. } => {
            assert_eq!(mode, crate::config::config_file::ListenAccess::Localhost)
        }
        _ => panic!("expected access"),
    }
}

#[test]
fn ifaces_parses_names_and_flags() {
    let m = Cli::command()
        .try_get_matches_from(to_os(&[
            "meshtown",
            "ifaces",
            "br-lan",
            "--if-unset",
            "--config",
            "/x",
        ]))
        .expect("ifaces parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Ifaces {
            names,
            if_unset,
            config,
            ..
        } => {
            assert_eq!(names, ["br-lan"]);
            assert!(if_unset);
            assert_eq!(config, Some(std::path::PathBuf::from("/x")));
        }
        _ => panic!("expected ifaces"),
    }
    let m = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "ifaces", "auto"]))
        .expect("ifaces auto parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Ifaces {
            names, if_unset, ..
        } => assert_eq!((names, if_unset), (vec!["auto".to_string()], false)),
        _ => panic!("expected ifaces"),
    }
}

#[test]
fn ifaces_without_names_is_error() {
    assert!(Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "ifaces"]))
        .is_err());
}

#[test]
fn access_bogus_mode_is_bad_value_with_choices() {
    let e = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "access", "everyone"]))
        .expect_err("invalid mode");
    assert_eq!(e.kind(), ErrorKind::ValueValidation);
    let text = crate::cli::cli_errors::bad_value_text(&e, Some("access"));
    assert!(
        text.contains("localhost") && text.contains("localnet"),
        "{text}"
    );
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
    fn default_subcommand_without_tray_is_run() {
        // given
        let args = to_os(&["meshtown"]);

        // when
        let got = with_default_subcommand(args, false);

        // then
        assert_eq!(got.get(1), Some(&OsString::from("run")));
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
        assert!(after_help_text(false).contains("Mesh Town starts"));
    }
}

#[test]
fn open_parses_plain_and_with_flags() {
    let m = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "open"]))
        .expect("open parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Open {
            print, web_port, ..
        } => assert!(!print && web_port.is_none()),
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
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Open {
            print, web_port, ..
        } => assert!(print && web_port == Some(7778)),
        _ => panic!("expected open"),
    }
}
