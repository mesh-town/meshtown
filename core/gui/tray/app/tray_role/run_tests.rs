//! Tests for starting the `tray` subcommand: the child's arguments, the token path, the link, the runtime check (`run.rs`).

use super::*;
use crate::gui::tray::app::proxy_state::ProxyOff;

use crate::gui::tray::app::local_api::tests::temp_dir;

#[derive(clap::Parser)]
struct Cli {
    #[command(flatten)]
    args: TrayArgs,
}

fn parse(argv: &[&str]) -> Result<TrayArgs, clap::Error> {
    use clap::Parser as _;
    let full = std::iter::once("tray").chain(argv.iter().copied());
    Cli::try_parse_from(full).map(|c| c.args)
}

#[test]
fn child_args_empty_is_run() {
    let args = TrayArgs {
        config: None,
        web_port: None,
        run_args: Vec::new(),
    };
    assert_eq!(child_args(&args), vec![OsString::from("run")]);
}

#[test]
fn child_args_order() {
    let args = parse(&[
        "--config",
        "/c/mesh.toml",
        "--web-port",
        "7071",
        "--",
        "--key",
        "K",
        "--no-socks",
    ])
    .expect("parse");
    let want: Vec<OsString> = [
        "run",
        "--config",
        "/c/mesh.toml",
        "--web-port",
        "7071",
        "--key",
        "K",
        "--no-socks",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    assert_eq!(child_args(&args), want);
}

#[test]
fn web_port_zero_rejected() {
    let err = parse(&["--web-port", "0"])
        .err()
        .expect("0 is not accepted");
    assert_eq!(err.exit_code(), 2);
    assert!(parse(&["--web-port", "65535"]).is_ok());
}

#[test]
fn token_path_next_to_config() {
    assert_eq!(
        token_path_for(Path::new("/c/mesh.toml")),
        Path::new("/c/web-token")
    );
    assert_eq!(
        token_path_for(Path::new("mesh.toml")),
        Path::new("./web-token")
    );
}

#[test]
fn link_for_reads_token_file() {
    let dir = temp_dir("link");
    let path = dir.join("web-token");
    std::fs::write(&path, "TOKEN\n").unwrap();
    let want = format!("http://127.0.0.1:7070/?token={}", "TOKEN");
    let got = page_url_for(7070, &path);
    assert!(
        got.as_deref() == Some(want.as_str()),
        "the link did not match"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn link_for_missing_file_is_none() {
    let dir = temp_dir("link-none");
    assert!(page_url_for(7070, &dir.join("web-token")).is_none());
    std::fs::write(dir.join("web-token"), "  \n").unwrap();
    assert!(page_url_for(7070, &dir.join("web-token")).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn runtime_ok_current_thread_false() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert!(!is_multi_thread_runtime(rt.handle()));
}

#[test]
fn runtime_ok_multi_thread_true() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .unwrap();
    assert!(is_multi_thread_runtime(rt.handle()));
}

fn os(argv: &[&str]) -> Vec<OsString> {
    argv.iter().map(OsString::from).collect()
}

#[test]
fn socks_without_flags_follows_settings() {
    assert_eq!(proxy_plan_for(1080, true, &[]), Ok(1080));
    let other = os(&["--key", "K", "--no-checks"]);
    assert_eq!(proxy_plan_for(1090, true, &other), Ok(1090));
    assert_eq!(
        proxy_plan_for(1080, false, &[]),
        Err(ProxyOff::DisabledInSettings)
    );
    assert_eq!(
        proxy_plan_for(1090, false, &other),
        Err(ProxyOff::DisabledInSettings)
    );
}

#[test]
fn socks_port_flag_beats_settings_port() {
    assert_eq!(
        proxy_plan_for(1080, true, &os(&["--socks-port", "1081"])),
        Ok(1081)
    );
    assert_eq!(
        proxy_plan_for(1080, true, &os(&["--socks-port=1082"])),
        Ok(1082)
    );
    let mixed = os(&["--key", "K", "--socks-port", "1083", "--no-checks"]);
    assert_eq!(proxy_plan_for(1080, true, &mixed), Ok(1083));
}

#[test]
fn socks_port_flag_does_not_enable_the_proxy() {
    // Without `--socks` the settings file decides: the port is named, but the proxy stays off as it was.
    assert_eq!(
        proxy_plan_for(1080, false, &os(&["--socks-port", "1081"])),
        Err(ProxyOff::DisabledInSettings)
    );
}

#[test]
fn no_socks_flag_beats_enabled_settings() {
    assert_eq!(
        proxy_plan_for(1080, true, &os(&["--no-socks"])),
        Err(ProxyOff::NoSocksFlag)
    );
    // A proxy that is off does not need a port, even a named one.
    let with_port = os(&["--no-socks", "--socks-port", "1081"]);
    assert_eq!(
        proxy_plan_for(1080, true, &with_port),
        Err(ProxyOff::NoSocksFlag)
    );
}

#[test]
fn socks_flag_beats_disabled_settings() {
    assert_eq!(proxy_plan_for(1080, false, &os(&["--socks"])), Ok(1080));
    let with_port = os(&["--socks", "--socks-port", "1081"]);
    assert_eq!(proxy_plan_for(1080, false, &with_port), Ok(1081));
}

#[test]
fn last_of_socks_and_no_socks_wins() {
    // `clap` settles the dispute (`overrides_with`), as in `run`: the one typed last wins.
    assert_eq!(
        proxy_plan_for(1080, true, &os(&["--socks", "--no-socks"])),
        Err(ProxyOff::NoSocksFlag)
    );
    assert_eq!(
        proxy_plan_for(1080, false, &os(&["--no-socks", "--socks"])),
        Ok(1080)
    );
}

#[test]
fn socks_flags_are_what_the_child_gets() {
    // The menu names what `run` will see in the child's arguments.
    let port = parse(&["--", "--socks-port", "1081"]).expect("parse");
    assert_eq!(proxy_plan_for(1080, true, &port.run_args), Ok(1081));
    assert!(child_args(&port).contains(&OsString::from("1081")));
    let off = parse(&["--", "--no-socks"]).expect("parse");
    assert_eq!(
        proxy_plan_for(1080, true, &off.run_args),
        Err(ProxyOff::NoSocksFlag)
    );
    assert!(child_args(&off).contains(&OsString::from("--no-socks")));
}

#[test]
fn unparsable_flags_fall_back_to_settings() {
    // `run` does not accept anything below 1024, nor garbage or an unknown flag: the node will say why,
    // and the menu takes the port and the on/off state from the file — even if the same parameters contain `--no-socks`.
    for bad in [
        &["--socks-port", "80"][..],
        &["--socks-port", "abc"],
        &["--socks-port"],
        &["--no-such-flag", "--socks-port", "1081"],
        &["--no-socks", "--no-such-flag"],
    ] {
        assert_eq!(proxy_plan_for(1080, true, &os(bad)), Ok(1080), "{bad:?}");
        assert_eq!(
            proxy_plan_for(1080, false, &os(bad)),
            Err(ProxyOff::DisabledInSettings),
            "{bad:?}"
        );
    }
}

#[test]
fn access_flag_after_dashes_beats_settings() {
    assert_eq!(
        proxy_access_for(ListenAccess::Localhost, &[]),
        ListenAccess::Localhost
    );
    assert_eq!(
        proxy_access_for(ListenAccess::Localnet, &[]),
        ListenAccess::Localnet
    );
    let flag = os(&["--socks-access", "localnet"]);
    assert_eq!(
        proxy_access_for(ListenAccess::Localhost, &flag),
        ListenAccess::Localnet
    );
    let bad = os(&["--socks-access", "anywhere"]);
    assert_eq!(
        proxy_access_for(ListenAccess::Localnet, &bad),
        ListenAccess::Localnet,
        "parsing failed — taken from the file"
    );
}
