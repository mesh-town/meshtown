//! Tests of the default subcommand substitution (see `cli.rs`): starting with no arguments behaves
//! like `tray` (a build with the tray) or `run` (without it); explicit subcommands and flags don't change.

use std::ffi::OsString;

use clap::error::ErrorKind;
use clap::FromArgMatches;

use crate::cli::{with_default_subcommand, Cli, CliCommand};

/// `&str` → `OsString`, shorter at the call site.
fn to_os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[cfg(not(feature = "tray"))]
#[test]
fn no_args_is_run_without_tray() {
    use crate::cli::is_flag_typed;

    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&["meshtown"])))
        .expect("no subcommand and no tray — run");
    let run_m = m
        .subcommand_matches("run")
        .expect("run subcommand substituted");
    assert!(
        !is_flag_typed(run_m, "port"),
        "the default must not look typed"
    );
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::Run(_)
    ));
}

#[cfg(feature = "tray")]
#[test]
fn no_args_is_tray_with_tray() {
    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&["meshtown"])))
        .expect("no subcommand and with tray — tray");
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::Tray(_)
    ));
}

#[test]
fn explicit_subcommand_kept() {
    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&["meshtown", "keygen"])))
        .expect("keygen parses");
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::Keygen { .. }
    ));

    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&[
            "meshtown",
            "config-path",
        ])))
        .expect("config-path parses");
    assert!(matches!(
        Cli::from_arg_matches(&m).expect("parse").command,
        CliCommand::ConfigPath { .. }
    ));

    let m = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&[
            "meshtown",
            "run",
            "--port",
            "7000",
        ])))
        .expect("run --port 7000 parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Run(args) => assert_eq!(args.port, Some(7000)),
        _ => panic!("expected the run subcommand"),
    }
}

#[test]
fn flags_without_subcommand_still_error() {
    let err = Cli::command()
        .try_get_matches_from(with_default_subcommand(to_os(&[
            "meshtown",
            "--port",
            "1",
        ])))
        .expect_err("a flag without a subcommand must not silently become run");
    assert_ne!(err.kind(), ErrorKind::DisplayHelp);
}

#[test]
fn empty_argv_untouched() {
    assert!(with_default_subcommand(Vec::<OsString>::new()).is_empty());
}

#[test]
fn password_reset_and_access_parse() {
    let m = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "password", "reset", "--web-port", "7001"]))
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
            assert_eq!(mode, meshtown::config_file::ListenAccess::Localhost)
        }
        _ => panic!("expected access"),
    }
}

#[test]
fn ifaces_parses_names_and_flags() {
    let m = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "ifaces", "br-lan", "--if-unset", "--config", "/x"]))
        .expect("ifaces parses");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Ifaces { names, if_unset, config, .. } => {
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
        CliCommand::Ifaces { names, if_unset, .. } => assert_eq!((names, if_unset), (vec!["auto".to_string()], false)),
        _ => panic!("expected ifaces"),
    }
}

#[test]
fn ifaces_without_names_is_error() {
    assert!(Cli::command().try_get_matches_from(to_os(&["meshtown", "ifaces"])).is_err());
}

#[test]
fn access_bogus_mode_is_bad_value_with_choices() {
    let e = Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "access", "everyone"]))
        .expect_err("invalid mode");
    assert_eq!(e.kind(), ErrorKind::ValueValidation);
    let text = crate::cli_errors::bad_value_text(&e, Some("access"));
    assert!(text.contains("localhost") && text.contains("localnet"), "{text}");
}

#[test]
fn password_needs_the_reset_word() {
    assert!(Cli::command()
        .try_get_matches_from(to_os(&["meshtown", "password", "forget"]))
        .is_err());
}
