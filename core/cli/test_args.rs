//! Helpers shared by the command line tests: argument lists, a refused parse and an accepted one.

use std::ffi::OsString;

use clap::{ArgMatches, FromArgMatches};

use crate::cli::args::{Cli, Globals};

/// `&str` to `OsString`, shorter at the call site.
pub(crate) fn to_os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// The error clap gives for `meshtown <args>`; the arguments must be refused.
pub(crate) fn parse_err(args: &[&str]) -> clap::Error {
    let mut all = vec!["meshtown"];
    all.extend_from_slice(args);
    Cli::command()
        .try_get_matches_from(all)
        .expect_err("parsing must refuse")
}

/// The parsed `meshtown <args>` and the matches it came from; the arguments must be accepted.
pub(crate) fn parse_ok(args: &[&str]) -> (Cli, ArgMatches) {
    let mut all = vec!["meshtown"];
    all.extend_from_slice(args);
    let matches = Cli::command()
        .try_get_matches_from(all)
        .expect("parsing must accept");
    let cli = Cli::from_arg_matches(&matches).expect("the matches fit `Cli`");
    (cli, matches)
}

/// The global options of `meshtown <args>`.
pub(crate) fn globals_of(args: &[&str]) -> Globals {
    parse_ok(args).0.globals
}
