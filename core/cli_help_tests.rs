//! Tests of the English help of `meshtown --help` and the subcommand helps (see `cli.rs`, `commands/run_args.rs`).

use clap::error::ErrorKind;

use crate::cli::Cli;

const ABOUT: &str = "Mesh Town — open sites through the computers of your network's members.";

fn top() -> String {
    Cli::command().render_help().to_string()
}

fn top_long() -> String {
    Cli::command().render_long_help().to_string()
}

/// Subcommand help as the program prints it: through the `DisplayHelp` error.
fn sub_help(name: &str) -> String {
    let err = Cli::command()
        .try_get_matches_from(["meshtown", name, "--help"])
        .expect_err("--help ends parsing");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    err.render().to_string()
}

fn sub_names() -> Vec<&'static str> {
    let mut v = vec!["run", "keygen", "config-path", "password", "access", "ifaces"];
    if cfg!(feature = "tray") {
        v.push("tray");
    }
    v
}

#[test]
fn top_help_speaks_english() {
    for help in [top(), top_long()] {
        for want in [
            ABOUT,
            "Usage: meshtown [COMMAND]",
            "Commands:",
            "Options:",
            "Run the node in this window, without an icon",
            "Print a new network key",
            "Print the path to the settings file",
            "Reset the node page password",
            "Who can open the node page",
            "Show help (for one command: meshtown run --help)",
            "Show version",
        ] {
            assert!(help.contains(want), "no \"{want}\" in the help:\n{help}");
        }
    }
    #[cfg(feature = "tray")]
    assert!(top().contains("Show the Mesh Town icon next to the clock and start the node"));
}

#[test]
fn top_help_has_no_clap_defaults() {
    for help in [top(), top_long()] {
        for bad in ["Print help", "Print version", "[env:", "<COMMAND>", "  help "] {
            assert!(!help.contains(bad), "\"{bad}\" in the help:\n{help}");
        }
    }
}

#[test]
fn top_help_headings_appear_once() {
    for help in [top(), top_long()] {
        for heading in ["Usage:", "Commands:", "Options:"] {
            assert_eq!(
                help.matches(heading).count(),
                1,
                "\"{heading}\" must appear once in the help:\n{help}"
            );
        }
    }
}

#[test]
fn commands_in_order() {
    let help = top();
    let mut names = vec!["  run ", "  keygen ", "  config-path ", "  password ", "  access ", "  ifaces "];
    if cfg!(feature = "tray") {
        names.insert(0, "  tray ");
    }
    let pos: Vec<usize> = names
        .iter()
        .map(|n| help.find(n).unwrap_or_else(|| panic!("no \"{n}\":\n{help}")))
        .collect();
    assert!(
        pos.windows(2).all(|w| w[0] < w[1]),
        "command order is broken:\n{help}"
    );
}

#[cfg(feature = "tray")]
#[test]
fn footer_names_default() {
    let help = top();
    assert!(
        help.contains("With no command, the Mesh Town icon starts next to the clock."),
        "{help}"
    );
    assert!(
        help.contains("Join a network with a member's invite:"),
        "{help}"
    );
    assert!(
        help.contains("meshtown run --invite \"<invite>\""),
        "{help}"
    );
}

#[cfg(not(feature = "tray"))]
#[test]
fn footer_names_default() {
    let help = top();
    assert!(help.contains("With no command, the node starts."), "{help}");
    assert!(!help.contains("  tray "), "{help}");
    assert!(
        help.contains("meshtown run --invite \"<invite>\""),
        "{help}"
    );
}

#[test]
fn sub_help_speaks_english() {
    for name in sub_names() {
        let help = sub_help(name);
        assert!(
            help.contains(&format!("Usage: meshtown {name}")),
            "{name}:\n{help}"
        );
        for heading in ["Usage:", "Options:"] {
            assert_eq!(help.matches(heading).count(), 1, "{name}: \"{heading}\":\n{help}");
        }
        for bad in ["Arguments:", "Print help"] {
            assert!(!help.contains(bad), "{name}: \"{bad}\":\n{help}");
        }
    }
}

/// Value names allowed in the help: from the dictionary (`<KEY>`, `<SECONDS>`…).
const LABELS: [&str; 8] = ["TEXT", "KEY", "PATH", "SECONDS", "SITE", "ADDR:PORT", "PORT", "MODE"];

/// Labels in angle brackets in the text that are not among the allowed value names.
fn unknown_labels(s: &str) -> Vec<&str> {
    s.split('<')
        .skip(1)
        .filter_map(|tail| tail.split_once('>').map(|(label, _)| label))
        .filter(|label| !LABELS.contains(label))
        .collect()
}

#[test]
fn run_help_words() {
    let help = sub_help("run");
    for want in [
        "Only via a relay server, no direct connections",
        "Add a site to your list (for example example.com); it is checked at startup. Can be repeated",
        "can be set with the MESHTOWN_KEY variable",
        "can be set with the MESHTOWN_INVITE variable",
        "can be set with the MESHTOWN_CONFIG variable",
    ] {
        assert!(help.contains(want), "no \"{want}\":\n{help}");
    }
    assert!(
        help.find("--invite").expect("--invite") < help.find("--key").expect("--key"),
        "{help}"
    );
    assert!(!help.contains("[env:"), "{help}");
    assert_eq!(unknown_labels(&help), Vec::<&str>::new(), "{help}");
    // The flag names are `--domain`, `--config`; the descriptions say "site" and "settings file".
    let prose: Vec<&str> = help
        .lines()
        .filter_map(|l| l.trim_start().split_once("  ").map(|(_, d)| d))
        .collect();
    for bad in ["domain", "config"] {
        assert!(prose.iter().all(|d| !d.contains(bad)), "\"{bad}\":\n{help}");
    }
}

#[test]
fn run_help_hides_port_flag() {
    // given: the `run` help; the `--port` flag has had no effect since 0.1.38, it is accepted for old command lines
    let help = sub_help("run");
    // then: no help line describes `--port` (flags start a line after the indent)
    let listed: Vec<&str> = help
        .lines()
        .filter(|l| l.trim_start().starts_with("--port"))
        .collect();
    assert!(listed.is_empty(), "the --port flag is visible in the help: {listed:?}\n{help}");
}

#[test]
fn help_flags_still_display() {
    for args in [
        vec!["-h"],
        vec!["--help"],
        vec!["run", "--help"],
        vec!["run", "-h"],
    ] {
        let argv: Vec<&str> = std::iter::once("meshtown")
            .chain(args.iter().copied())
            .collect();
        let err = Cli::command()
            .try_get_matches_from(&argv)
            .expect_err("help");
        assert_eq!(err.kind(), ErrorKind::DisplayHelp, "{argv:?}");
    }
    for flag in ["-V", "--version"] {
        let err = Cli::command()
            .try_get_matches_from(["meshtown", flag])
            .expect_err("version");
        assert_eq!(err.kind(), ErrorKind::DisplayVersion, "{flag}");
    }
}

#[test]
fn help_word_is_not_a_command() {
    let err = Cli::command()
        .try_get_matches_from(["meshtown", "help"])
        .expect_err("help is not a command");
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
}

#[test]
fn env_hidden_but_read() {
    let cmd = Cli::command();
    let run = cmd.find_subcommand("run").expect("run");
    for (id, var) in [
        ("key", "MESHTOWN_KEY"),
        ("invite", "MESHTOWN_INVITE"),
        ("config", "MESHTOWN_CONFIG"),
    ] {
        let arg = run.get_arguments().find(|a| a.get_id() == id).expect(id);
        assert_eq!(arg.get_env().and_then(|e| e.to_str()), Some(var), "{id}");
        assert!(
            arg.is_hide_env_set(),
            "{id}: the variable is not hidden in the help"
        );
    }
}
