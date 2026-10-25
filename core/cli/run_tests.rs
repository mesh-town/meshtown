//! Tests of parsing the `run` subcommand flags (see `main.rs`, `cli.rs`, `commands.rs`).

use clap::error::ErrorKind;
use clap::{ArgMatches, FromArgMatches};

use crate::cli::args::{is_flag_typed, toggle_from_flag_pair, Cli, CliCommand};

/// The identifiers `run` checks through `is_flag_typed`.
const TYPED_IDS: [&str; 7] = [
    "socks_port",
    "check_interval",
    "check_doh",
    "provider_dns",
    "probe_interval",
    "config",
    "web_port",
];

/// Parses `meshtown run <args>`; returns the matches of the `run` subcommand.
fn run_matches(args: &[&str]) -> Result<ArgMatches, clap::Error> {
    let mut all = vec!["meshtown", "run"];
    all.extend_from_slice(args);
    let m = Cli::command().try_get_matches_from(all)?;
    Ok(m.subcommand_matches("run").expect("run subcommand").clone())
}

/// `MESHTOWN_KEY` in the runner's environment makes `value_source("key")` equal `EnvVariable`.
fn skip_for_mesh_key(test: &str) -> bool {
    if std::env::var_os("MESHTOWN_KEY").is_some() {
        eprintln!("test {test} skipped: MESHTOWN_KEY is set in the environment");
        return true;
    }
    false
}

/// The pair `--socks / --no-socks` after parsing, as `run` sees it.
fn socks_pair(args: &[&str]) -> Option<bool> {
    let mut all = vec!["meshtown", "run"];
    all.extend_from_slice(args);
    let m = Cli::command().try_get_matches_from(all).expect("parse");
    match Cli::from_arg_matches(&m).expect("parse").command {
        CliCommand::Run(args) => toggle_from_flag_pair(args.socks, args.no_socks),
        _ => panic!("expected the run subcommand"),
    }
}

#[test]
fn default_flags_not_typed() {
    if skip_for_mesh_key("default_flags_not_typed") {
        return;
    }
    let m = run_matches(&[]).expect("parse");
    for id in TYPED_IDS {
        assert!(
            !is_flag_typed(&m, id),
            "{id} was not typed, but typed says yes"
        );
    }
}

#[test]
fn typed_flag_detected() {
    let m = run_matches(&["--socks-port", "1081"]).expect("parse");
    for id in TYPED_IDS {
        assert_eq!(is_flag_typed(&m, id), id == "socks_port", "identifier {id}");
    }
}

#[test]
fn web_port_flag_detected() {
    let m = run_matches(&["--web-port", "8080"]).expect("parse");
    for id in TYPED_IDS {
        assert_eq!(is_flag_typed(&m, id), id == "web_port", "identifier {id}");
    }
}

#[test]
fn no_config_conflicts_save_config() {
    let e = run_matches(&["--no-config", "--save-config"]).expect_err("there must be a conflict");
    assert_eq!(e.kind(), ErrorKind::ArgumentConflict);
}

#[test]
fn relay_flags_conflict() {
    assert!(run_matches(&["--relay-only", "--no-relay"]).is_err());
}

#[test]
fn pair_last_wins() {
    assert_eq!(socks_pair(&["--socks", "--no-socks"]), Some(false));
    assert_eq!(socks_pair(&["--no-socks", "--socks"]), Some(true));
}

#[test]
fn key_optional() {
    if skip_for_mesh_key("key_optional") {
        return;
    }
    assert!(run_matches(&[]).is_ok());
}

#[test]
fn invite_and_key_parse_together() {
    if skip_for_mesh_key("invite_and_key_parse_together") {
        return;
    }
    let m = run_matches(&["--key", "K", "--invite", "mesh-invite2-aa"]).expect("parse");
    assert_eq!(m.get_one::<String>("key").map(String::as_str), Some("K"));
    assert_eq!(
        m.get_one::<String>("invite").map(String::as_str),
        Some("mesh-invite2-aa")
    );
}

#[cfg(feature = "gui")]
mod require_tray {
    use super::super::{require_tray, TrayRef};

    #[test]
    fn tray_without_backend_is_an_error() {
        // given no tray backend, when it is required
        let err = require_tray(None)
            .err()
            .expect("a build without a tray must fail");

        // then the error says there is no tray icon
        assert!(
            err.to_string().contains("has no tray icon"),
            "unexpected text: {err}"
        );
    }

    #[cfg(all(
        feature = "net-hyper",
        any(target_os = "macos", target_os = "windows", target_os = "linux")
    ))]
    #[test]
    fn tray_with_backend_is_returned() {
        let tray: TrayRef = &crate::gui::tray::test_tray::TEST_TRAY;

        let got = require_tray(Some(tray)).expect("a present backend is returned");

        assert!(std::ptr::addr_eq(got, tray), "the same backend comes back");
    }
}

mod log_filter {
    use super::super::log_filter;
    use tracing::Level;
    use tracing_subscriber::filter::Targets;

    fn on(filter: &Targets, target: &str, level: Level) -> bool {
        filter.would_enable(target, &level)
    }

    #[test]
    fn unset_rust_log_gives_warn() {
        let f = log_filter(None);

        assert!(on(&f, "meshtown_core", Level::WARN));
        assert!(!on(&f, "meshtown_core", Level::INFO));
    }

    #[test]
    fn bare_level_applies_to_all() {
        let f = log_filter(Some("info"));

        assert!(on(&f, "meshtown_core", Level::INFO));
        assert!(on(&f, "mainline::rpc", Level::INFO));
        assert!(!on(&f, "meshtown_core", Level::DEBUG));
    }

    #[test]
    fn target_list_values_parse() {
        let f = log_filter(Some("info,mainline::rpc::socket=off"));
        assert!(!on(&f, "mainline::rpc::socket", Level::ERROR));
        assert!(on(&f, "meshtown_core", Level::INFO));

        let f = log_filter(Some("warn,mainline::rpc::socket=error,bbr1_trace=info"));
        assert!(!on(&f, "mainline::rpc::socket", Level::WARN));
        assert!(on(&f, "mainline::rpc::socket", Level::ERROR));
        assert!(on(&f, "bbr1_trace", Level::INFO));
        assert!(on(&f, "other", Level::WARN));
        assert!(!on(&f, "other", Level::INFO));
    }

    #[test]
    fn invalid_value_falls_back_to_warn() {
        for raw in ["foo=bogus", "foo[{field=val}]=debug"] {
            let f = log_filter(Some(raw));

            assert!(on(&f, "foo", Level::WARN), "{raw}");
            assert!(!on(&f, "foo", Level::INFO), "{raw}");
        }
    }
}
