//! Helpers shared by the command line tests: argument lists and a refused parse.

use std::ffi::OsString;

use crate::cli::args::Cli;

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
