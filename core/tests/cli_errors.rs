//! Command-line errors of the real binary: the proxy port is busy, an unknown command, the `help` word.
//! The test holds the port itself (`127.0.0.1:0`), so it is not flaky; the node never reaches the network.

use std::process::{Command, Output};

use meshtown_core::invite::{INVITE_PREFIX, TEXT_V3_INVITE};
use meshtown_core::members::network_key::LEGACY_KEY_TEXT;

fn mesh(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_meshtown"))
        .args(args)
        .env("TOWNJUMP_BOOTSTRAP", "127.0.0.1:9")
        .env_remove("MESHTOWN_KEY")
        .env_remove("MESHTOWN_INVITE")
        .output()
        .expect("start meshtown")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn busy_socks_port_names_flag_and_port() {
    let holder = std::net::TcpListener::bind("127.0.0.1:0").expect("port");
    let port = holder.local_addr().unwrap().port().to_string();
    let key = text(&mesh(&["keygen", "--raw"]).stdout).trim().to_string();
    let out = mesh(&[
        "run",
        "--no-config",
        "--key",
        &key,
        "--web-port",
        "0",
        "--no-checks",
        "--socks-port",
        &port,
    ]);
    let err = text(&out.stderr);
    let want = format!("Error: port {port} for the browser proxy is in use by another program");
    assert!(err.contains(&want), "stderr: {err}");
    assert!(err.contains("--socks-port"), "stderr: {err}");
    assert_eq!(err.matches("Error:").count(), 1, "stderr: {err}");
    assert!(!err.contains("Caused by"), "stderr: {err}");
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn bogus_command() {
    let out = mesh(&["bogus"]);
    let err = text(&out.stderr);
    assert!(err.contains("unknown command \"bogus\""), "stderr: {err}");
    assert!(err.contains("Commands: "), "stderr: {err}");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn help_word_prints_help() {
    let out = mesh(&["help"]);
    assert!(text(&out.stdout).contains("Usage:"));
    assert_eq!(out.status.code(), Some(0));
}

// ---- Texts of an invalid option and the `help` word ----

/// An invalid option and an invalid value: one line in English in stderr, code 2, empty stdout.
mod bad_parameter {
    use super::*;

    mod unknown_flag {
        use super::*;

        #[test]
        fn in_run_points_to_run_help() {
            let out = mesh(&["run", "--bogus-flag"]);
            assert_refused_with(
                &out,
                "Error: unknown option \"--bogus-flag\". See all options: meshtown run --help",
            );
        }

        #[test]
        fn in_keygen_points_to_keygen_help() {
            let out = mesh(&["keygen", "--bogus-flag"]);
            assert_refused_with(
                &out,
                "Error: unknown option \"--bogus-flag\". See all options: meshtown keygen --help",
            );
        }
    }

    mod bad_value {
        use super::*;

        #[test]
        fn port_below_range_names_range() {
            let out = mesh(&["run", "--socks-port", "80"]);
            assert_refused_with(
                &out,
                "Error: invalid value \"80\" for --socks-port: use a number from 1024 to 65535. See all options: meshtown run --help",
            );
        }
    }
}

/// The `help` word and `help <command>` print to stdout the same as `--help` and `<command> --help`.
mod help_word {
    use super::*;

    #[test]
    fn alone_prints_what_help_flag_prints() {
        assert_same_help(&["help"], &["--help"]);
    }

    #[test]
    fn alone_ends_without_extra_blank_line() {
        let out = mesh(&["help"]);
        assert!(!out.stdout.ends_with(b"\n\n"), "{}", text(&out.stdout));
    }

    #[test]
    fn run_prints_what_run_help_flag_prints() {
        assert_same_help(&["help", "run"], &["run", "--help"]);
    }

    #[test]
    fn keygen_prints_what_keygen_help_flag_prints() {
        assert_same_help(&["help", "keygen"], &["keygen", "--help"]);
    }

    #[test]
    fn config_path_prints_what_its_help_flag_prints() {
        assert_same_help(&["help", "config-path"], &["config-path", "--help"]);
    }

    // On macOS `tray --help` with a non-terminal stderr goes to `meshtown-tray.log` (`app/console/macos.rs`,
    // accepted behavior): stdout is empty, there is nothing to compare — that check is manual.
    #[cfg(all(feature = "gui", not(target_os = "macos")))]
    #[test]
    fn tray_prints_what_tray_help_flag_prints() {
        assert_same_help(&["help", "tray"], &["tray", "--help"]);
    }

    #[test]
    fn unknown_command_says_no_such_command() {
        let out = mesh(&["help", "bogus"]);
        let err = text(&out.stderr);
        assert!(err.contains("unknown command \"bogus\""), "stderr: {err}");
        assert_eq!(err, text(&mesh(&["bogus"]).stderr), "same as for `bogus`");
        assert!(out.stdout.is_empty(), "stdout: {}", text(&out.stdout));
        assert_eq!(out.status.code(), Some(2));
    }
}

// ---- helpers of the new tests ----

/// Parse refusal: code 2, empty stdout, exactly one line `want` in stderr.
fn assert_refused_with(out: &Output, want: &str) {
    assert_eq!(out.status.code(), Some(2), "stderr: {}", text(&out.stderr));
    assert!(out.stdout.is_empty(), "stdout: {}", text(&out.stdout));
    assert_eq!(text(&out.stderr), format!("{want}\n"));
}

/// `word` (with the `help` command) prints to stdout byte for byte the same as `flag` (with `--help`), code 0.
fn assert_same_help(word: &[&str], flag: &[&str]) {
    let (by_word, by_flag) = (mesh(word), mesh(flag));
    assert_eq!(by_flag.status.code(), Some(0), "{flag:?}");
    assert_eq!(by_word.status.code(), Some(0), "{word:?}");
    assert!(!by_flag.stdout.is_empty(), "{flag:?}: empty help");
    assert!(
        by_word.stderr.is_empty(),
        "stderr: {}",
        text(&by_word.stderr)
    );
    assert!(
        by_word.stdout == by_flag.stdout,
        "{word:?} and {flag:?} print different output:\n--- {word:?}\n{}\n--- {flag:?}\n{}",
        text(&by_word.stdout),
        text(&by_flag.stdout)
    );
}

// ---- Outdated keys and invites ----

#[test]
fn a_version_3_invite_exits_with_the_outdated_text() {
    // given: an invite string whose version byte is 3
    let invite = format!(
        "{INVITE_PREFIX}{}",
        data_encoding::BASE32_NOPAD
            .encode(&[3, 1, 2, 3])
            .to_ascii_lowercase()
    );
    // when
    let out = mesh(&[
        "run",
        "--no-config",
        "--invite",
        &invite,
        "--web-port",
        "0",
        "--no-checks",
        "--no-socks",
    ]);
    // then
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains(TEXT_V3_INVITE), "stderr: {err}");
}

#[test]
fn no_config_with_an_old_key_exits_with_the_outdated_text() {
    // given: an old-form key and no settings file to fall back on
    let key = format!("mesh1-{}", "a".repeat(52));
    // when
    let out = mesh(&[
        "run",
        "--no-config",
        "--key",
        &key,
        "--web-port",
        "0",
        "--no-checks",
        "--no-socks",
    ]);
    // then
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "stderr: {err}");
    assert!(err.contains(LEGACY_KEY_TEXT), "stderr: {err}");
}
