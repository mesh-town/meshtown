//! Commands and options that were replaced: what to type now. The error texts use it after a refused parse,
//! so a name that is still alive never reaches this table.

use std::ffi::OsString;

/// A replaced command or option: the words typed in order (the first is the command) and what to type now.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Renamed {
    pub old: &'static [&'static str],
    /// The replacement as a command line without the program name.
    pub new: &'static str,
}

const fn row(old: &'static [&'static str], new: &'static str) -> Renamed {
    Renamed { old, new }
}

/// Every replaced name.
pub(super) const RENAMED: &[Renamed] = &[
    row(&["keygen"], "network create --key-only"),
    row(&["config-path"], "config path"),
    row(&["ifaces"], "access ifaces"),
    row(&["access", "localhost"], "access page localhost"),
    row(&["access", "localnet"], "access page localnet"),
    row(&["run", "--domain"], "run --site"),
    row(&["run", "--block-domain"], "run --block-site"),
    row(&["run", "--save-config"], "config set <PATH> <VALUE>"),
    row(&["run", "--probe-interval"], "run --set probe.interval_sec=<SECONDS>"),
    row(&["run", "--probe-full-cycle"], "run --set probe.full_cycle=true"),
    row(&["run", "--check-interval"], "run --set checks.interval_sec=<SECONDS>"),
    row(&["run", "--check-doh"], "run --set checks.doh_server=<URL>"),
    row(&["run", "--provider-dns"], "run --set checks.provider_dns=<ADDR>"),
    row(&["run", "--checks"], "run --set checks.enabled=true"),
    row(&["run", "--no-checks"], "run --set checks.enabled=false"),
];

/// A typed word without the `=value` of an option.
fn word_of(arg: &OsString) -> &str {
    let text = arg.to_str().unwrap_or("");
    match text.starts_with("--") {
        true => text.split('=').next().unwrap_or(text),
        false => text,
    }
}

/// Whether the words of `old` stand in `words` in order.
fn in_order(old: &[&str], words: &[&str]) -> bool {
    let mut rest = words.iter();
    old.iter().all(|w| rest.any(|typed| typed == w))
}

/// The row whose words were typed in order; the first word is the command name, found after the program name.
pub(crate) fn renamed_hint(args: &[OsString]) -> Option<&'static Renamed> {
    let words: Vec<&str> = args.iter().skip(1).map(word_of).collect();
    RENAMED.iter().find(|r| in_order(r.old, &words))
}

#[cfg(test)]
#[path = "renamed_tests.rs"]
mod tests;
