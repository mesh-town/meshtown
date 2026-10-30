//! Tests of the global options (`globals.rs`): where they may stand, the range of `--web-port` and the refusals.

use super::*;
use crate::cli::args::is_flag_typed;
use crate::cli::cli_errors::{classify_for_tests as classify, ParseFailForTests as ParseFail};

use crate::cli::exit::exit_code;
use crate::cli::test_args::{globals_of, parse_err, parse_ok};
use clap::error::ErrorKind;

fn usage_text(result: anyhow::Result<()>) -> String {
    let e = result.expect_err("must be refused");
    assert_eq!(
        format!("{:?}", exit_code(&e)),
        format!("{:?}", std::process::ExitCode::from(2))
    );
    e.to_string()
}

#[test]
fn config_before_and_after_command_is_one_value() {
    let before = globals_of(&["--config", "/x/c.json", "status"]);
    let after = globals_of(&["status", "--config", "/x/c.json"]);
    assert_eq!(before.config, Some(PathBuf::from("/x/c.json")));
    assert_eq!(after.config, before.config);
}

#[test]
fn config_reads_meshtown_config_env() {
    let cmd = crate::cli::args::Cli::command();
    let arg = cmd
        .get_arguments()
        .find(|a| a.get_id() == "config")
        .expect("config");
    assert_eq!(
        arg.get_env().and_then(|e| e.to_str()),
        Some("MESHTOWN_CONFIG")
    );
    assert!(arg.is_hide_env_set());
}

#[test]
fn web_port_zero_is_usage_error_for_status() {
    let g = globals_of(&["status", "--web-port", "0"]);
    let text = usage_text(g.allow("status", &[Global::Net, Global::Json]));
    assert!(text.contains("use a port from 1 to 65535"), "{text}");
}

#[test]
fn web_port_zero_is_accepted_by_run() {
    let g = globals_of(&["run", "--web-port", "0"]);
    assert_eq!(g.web_port, Some(0));
    assert!(g.allow("run", &[]).is_ok());
}

#[test]
fn web_port_above_range_is_bad_value() {
    let err = parse_err(&["status", "--web-port", "65536"]);
    assert_eq!(err.kind(), ErrorKind::ValueValidation);
    assert_eq!(classify(&err), ParseFail::BadValue);
}

#[test]
fn run_web_port_before_command_is_typed() {
    let (_, m) = parse_ok(&["--web-port", "9000", "run"]);
    let run = m.subcommand_matches("run").expect("run");
    assert!(is_flag_typed(run, "web_port"));
}

#[test]
fn allow_refuses_json_naming_the_command() {
    let g = Globals {
        json: true,
        ..Globals::default()
    };
    let text = usage_text(g.allow("stop", &[]));
    assert!(text.starts_with("--json isn't used by "), "{text}");
    assert!(text.contains(" stop. See: "), "{text}");
    assert!(text.ends_with("stop --help"), "{text}");
}

#[test]
fn allow_passes_when_flag_absent() {
    assert!(Globals::default().allow("stop", &[]).is_ok());
    let g = Globals {
        json: true,
        ..Globals::default()
    };
    assert!(g.allow("status", &[Global::Json]).is_ok());
}

#[test]
fn require_yes_refuses_without_yes() {
    let text = usage_text(Globals::default().require_yes("leave"));
    assert!(
        text.contains("leave removes data; add --yes to confirm"),
        "{text}"
    );
}

#[test]
fn require_yes_passes_with_yes() {
    let g = Globals {
        yes: true,
        ..Globals::default()
    };
    assert!(g.require_yes("leave").is_ok());
}

#[cfg(feature = "gui")]
#[test]
fn tray_web_port_zero_is_usage_error() {
    let g = globals_of(&["tray", "--web-port", "0"]);
    let text = usage_text(g.allow("tray", &[]));
    assert!(text.contains("use a port from 1 to 65535"), "{text}");
    assert!(globals_of(&["tray", "--web-port", "65535"])
        .allow("tray", &[])
        .is_ok());
}
