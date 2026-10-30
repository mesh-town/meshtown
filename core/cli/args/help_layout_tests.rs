//! Tests of the help layout: the `-h, --help` line of subcommands, the order of `-h`/`-V`,
//! the length of `run --help` lines and the verbatim texts of `--invite`, `--key`, `--config`.
//! Included from `cli.rs`: `#[cfg(test)] #[path = "args_help_layout_tests.rs"] mod help_layout_tests;`.
//! It takes nothing from the neighboring test files: `help_of` and `sub_names` are short copies from
//! `cli_help_tests.rs` (they are private there), `Cli` comes from `crate::cli::args`.

use clap::error::ErrorKind;

use crate::cli::args::Cli;

const HELP_FLAG: &str = "-h, --help";
const VERSION_FLAG: &str = "-V, --version";
const SHOW_HELP: &str = "Show help";
const SHOW_VERSION: &str = "Show version";
const TOP_HELP_TEXT: &str = "Show help (for one command: meshtown run --help)";

/// Two terminal lines of 80 columns.
const LINE_LIMIT: usize = 160;

// The texts are fixed. `clap` drops the trailing period of a one-paragraph description, so
// the help lines are compared without it (`ends_with_text`).
const INVITE: &str = "Invite to a network: a single-use string mesh-invite2-… from a running member (can be set with the MESHTOWN_INVITE variable).";
const KEY: &str = "Network key: a words key or mesh2-… (can be set with the MESHTOWN_KEY variable) (default: from the settings file).";
const RUN_CONFIG: &str = "Settings file (JSON); can be set with the MESHTOWN_CONFIG variable (default: next to the program or in the standard location).";

/// Subcommand help: each has its own `-h, --help` line, with no reference to another command's help.
mod subcommand_help {
    use super::*;

    mod help_flag_line {
        use super::*;

        #[test]
        fn says_show_help() {
            for name in sub_names() {
                let help = sub_help(name);
                let line = line_of(&help, HELP_FLAG);
                assert!(
                    line.trim_end().ends_with(SHOW_HELP),
                    "{name}: the line \"{line}\" doesn't end with \"{SHOW_HELP}\""
                );
            }
        }

        #[test]
        fn does_not_point_to_another_help() {
            for name in sub_names() {
                let help = sub_help(name);
                assert!(!help.contains("help for a command"), "{name}:\n{help}");
            }
        }

        #[test]
        fn closes_the_options_section() {
            for name in sub_names() {
                let help = sub_help(name);
                let last = options_section(&help).pop().unwrap_or("");
                assert!(
                    last.trim_start().starts_with(HELP_FLAG),
                    "{name}: the \"Options:\" section ends with the line \"{last}\":\n{help}"
                );
            }
        }
    }
}

/// Top-level help: `-h, --help` explains where the command help is; `-V, --version` comes after it.
mod top_help {
    use super::*;

    mod help_flag_line {
        use super::*;

        #[test]
        fn points_to_run_help() {
            let line = line_of(&top(), HELP_FLAG);
            assert!(line.contains(TOP_HELP_TEXT), "{line}");
        }
    }

    mod version_flag_line {
        use super::*;

        #[test]
        fn says_show_version() {
            let line = line_of(&top(), VERSION_FLAG);
            assert!(line.trim_end().ends_with(SHOW_VERSION), "{line}");
        }
    }

    mod options_section {
        use super::*;

        #[test]
        fn lists_help_before_version() {
            let help = top();
            let help_at = help.find(HELP_FLAG).expect("-h, --help");
            let version_at = help.find(VERSION_FLAG).expect("-V, --version");
            assert!(help_at < version_at, "-V before -h:\n{help}");
        }

        #[test]
        fn ends_with_help_then_version() {
            let help = top();
            let section = options_section(&help);
            let n = section.len();
            assert!(
                n >= 2,
                "the \"Global options:\" section has fewer than two lines:\n{help}"
            );
            let (before_last, last) = (section[n - 2].trim_start(), section[n - 1].trim_start());
            assert!(
                before_last.starts_with(HELP_FLAG),
                "second to last \"{before_last}\""
            );
            assert!(last.starts_with(VERSION_FLAG), "last \"{last}\"");
        }
    }
}

/// `run --help`: lines fit into two terminal lines, the texts of `--invite`, `--key`, `--config` are fixed.
mod run_help {
    use super::*;

    #[test]
    fn no_line_is_longer_than_two_terminal_rows() {
        let help = sub_help("run");
        let long: Vec<String> = help
            .lines()
            .filter(|l| l.chars().count() > LINE_LIMIT)
            .map(|l| format!("{} chars: {l}", l.chars().count()))
            .collect();
        assert!(
            long.is_empty(),
            "lines longer than {LINE_LIMIT}:\n{}",
            long.join("\n")
        );
    }

    #[test]
    fn invite_line_has_contract_text() {
        let line = line_of(&sub_help("run"), "--invite");
        assert!(ends_with_text(&line, INVITE), "{line}");
    }

    #[test]
    fn key_line_has_contract_text() {
        let line = line_of(&sub_help("run"), "--key");
        assert!(ends_with_text(&line, KEY), "{line}");
    }

    #[test]
    fn config_line_has_contract_text() {
        let line = line_of(&sub_help("run"), "--config");
        assert!(ends_with_text(&line, RUN_CONFIG), "{line}");
    }
}

/// `config-path --help`: `--config` is a global option, the same text as in `run`.
mod config_path_help {
    use super::*;

    #[test]
    fn config_line_has_contract_text() {
        let line = line_of(&sub_help("config-path"), "--config");
        assert!(ends_with_text(&line, RUN_CONFIG), "{line}");
    }
}

// ---- helpers ----

#[cfg(feature = "gui")]
fn sub_names() -> Vec<&'static str> {
    vec!["tray", "run", "join", "status", "stop", "invite", "leave", "keygen", "config-path", "password", "access"]
}

#[cfg(not(feature = "gui"))]
fn sub_names() -> Vec<&'static str> {
    vec!["run", "join", "status", "stop", "invite", "leave", "keygen", "config-path", "password", "access"]
}

/// Help as the program prints it: through the `DisplayHelp` error on `--help`.
fn help_of(args: &[&str]) -> String {
    let mut argv = vec!["meshtown"];
    argv.extend_from_slice(args);
    let err = Cli::command()
        .try_get_matches_from(argv)
        .expect_err("--help ends parsing");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    err.render().to_string()
}

fn top() -> String {
    help_of(&["--help"])
}

fn sub_help(name: &str) -> String {
    help_of(&[name, "--help"])
}

/// Lines of the "Options:" section without blank ones: from the heading to the first blank line or the end of the text.
fn options_section(help: &str) -> Vec<&str> {
    help.lines()
        .skip_while(|l| *l != "Options:" && *l != "Global options:")
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .collect()
}

/// A help line starting (after the indent) with a flag; if there is none, the test fails with the help text.
fn line_of(help: &str, flag: &str) -> String {
    let start = format!("{flag} ");
    let found = help.lines().find(|l| l.trim_start().starts_with(&start));
    found
        .unwrap_or_else(|| panic!("no line with \"{flag}\" in the help:\n{help}"))
        .to_string()
}

/// The line ends with this description; the trailing period doesn't count (`clap` drops it).
fn ends_with_text(line: &str, text: &str) -> bool {
    let line = line.trim_end().trim_end_matches('.');
    line.ends_with(text.trim_end_matches('.'))
}
