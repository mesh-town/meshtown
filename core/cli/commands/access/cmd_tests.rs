use crate::cli::args::CliCommand;
use crate::cli::commands::access::{AccessArgs, AccessCmd};
use crate::cli::test_args::{parse_err, parse_ok};
use crate::config::config_file::ListenAccess;

fn access(args: &[&str]) -> AccessArgs {
    match parse_ok(args).0.command {
        CliCommand::Access(a) => a,
        _ => panic!("expected access"),
    }
}

#[test]
fn page_and_proxy_parse_mode() {
    match access(&["access", "page", "localhost"]).cmd {
        Some(AccessCmd::Page { mode }) => assert_eq!(mode, ListenAccess::Localhost),
        _ => panic!("expected access page"),
    }
    match access(&["access", "proxy", "localnet"]).cmd {
        Some(AccessCmd::Proxy { mode }) => assert_eq!(mode, ListenAccess::Localnet),
        _ => panic!("expected access proxy"),
    }
}

#[test]
fn bogus_mode_lists_choices() {
    let e = parse_err(&["access", "page", "everyone"]);
    assert_eq!(e.kind(), clap::error::ErrorKind::ValueValidation);
    let text = crate::cli::cli_errors::bad_value_text(&e, Some("access"));
    assert!(
        text.contains("localhost") && text.contains("localnet"),
        "{text}"
    );
}

#[test]
fn ifaces_parses_names_and_if_unset() {
    let (cli, _) = parse_ok(&["access", "ifaces", "br-lan", "--if-unset", "--config", "/x"]);
    assert_eq!(cli.globals.config, Some(std::path::PathBuf::from("/x")));
    match cli.command {
        CliCommand::Access(AccessArgs {
            cmd: Some(AccessCmd::Ifaces { names, if_unset }),
        }) => {
            assert_eq!(names, ["br-lan"]);
            assert!(if_unset);
        }
        _ => panic!("expected access ifaces"),
    }
}

#[test]
fn ifaces_auto_parses() {
    match access(&["access", "ifaces", "auto"]).cmd {
        Some(AccessCmd::Ifaces { names, if_unset }) => {
            assert_eq!((names, if_unset), (vec!["auto".to_string()], false))
        }
        _ => panic!("expected access ifaces"),
    }
}

#[test]
fn ifaces_without_names_is_error() {
    parse_err(&["access", "ifaces"]);
}

#[test]
fn access_without_action_parses_as_show() {
    assert!(access(&["access"]).cmd.is_none());
}

#[test]
fn old_form_access_localhost_is_unknown_subcommand() {
    let e = parse_err(&["access", "localhost"]);
    assert_eq!(e.kind(), clap::error::ErrorKind::InvalidSubcommand);
}

#[test]
fn nested_commands_have_their_own_help() {
    for args in [
        &["access", "page", "--help"][..],
        &["access", "ifaces", "-h"],
    ] {
        let e = parse_err(args);
        assert_eq!(e.kind(), clap::error::ErrorKind::DisplayHelp, "{args:?}");
        assert!(e.render().to_string().contains("-h, --help  "), "{args:?}");
    }
}
