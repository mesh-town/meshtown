//! Parsing of the demo node's `--cli-override path[=value]` into `CliOverrides`.

use meshtown_core::config::config_file::CliOverrides;

fn parse_bool_override(value: Option<&str>, default_no_value: bool, path: &str) -> bool {
    match value {
        None => default_no_value,
        Some("true") => true,
        Some("false") => false,
        Some(v) => {
            eprintln!("web_demo: --cli-override {path}: couldn't parse the value \"{v}\"");
            std::process::exit(2);
        }
    }
}

fn parse_u_or_exit<T: std::str::FromStr>(path: &str, v: &str) -> T {
    v.parse().unwrap_or_else(|_| {
        eprintln!("web_demo: --cli-override {path}: couldn't parse the value \"{v}\"");
        std::process::exit(2);
    })
}

/// `value` is absent (the flag has no `=value`) → `default`; otherwise it is parsed through `parse_u_or_exit`.
/// A shared template for the numeric and address fields of `CliOverrides`.
fn parse_with_default<T: std::str::FromStr>(path: &str, value: Option<&str>, default: T) -> T {
    match value {
        None => default,
        Some(v) => parse_u_or_exit(path, v),
    }
}

fn split_override_entry(entry: &str) -> (&str, Option<&str>) {
    match entry.split_once('=') {
        Some((p, v)) => (p, Some(v)),
        None => (entry, None),
    }
}

/// Path → a change to the matching `cli` field. The only place that parses the `--cli-override`
/// `--cli-override`; `node.name` has no slot here and is marked in
/// `prepare_demo_config` after step 7.
fn apply_cli_override(cli: &mut CliOverrides, path: &str, value: Option<&str>) {
    match path {
        "exit.enabled" => cli.exit_enabled = Some(parse_bool_override(value, false, path)),
        "socks.enabled" => cli.socks_enabled = Some(parse_bool_override(value, false, path)),
        "checks.enabled" => cli.checks_enabled = Some(parse_bool_override(value, false, path)),
        "socks.port" => cli.socks_port = Some(parse_with_default(path, value, 2000u16)),
        "checks.interval_sec" => {
            cli.check_interval_sec = Some(parse_with_default(path, value, 900u64));
        }
        "checks.doh_server" => {
            let default = "9.9.9.9:443".parse().expect("the built-in address parses");
            cli.doh_server = Some(parse_with_default(path, value, default));
        }
        "checks.provider_dns" => {
            let default = "8.8.8.8:53".parse().expect("the built-in address parses");
            cli.provider_dns = Some(parse_with_default(path, value, default));
        }
        "probe.interval_sec" => {
            cli.probe_interval_sec = Some(parse_with_default(path, value, 300u64));
        }
        // The node's flags have no such CliOverrides field: it is a pure "set by a start
        // flag" marker for the page, and the value (if any) is not used. It is handled
        // separately in `prepare_demo_config` after step 7 (`run.cli_override_paths.push(...)`).
        "node.name" => {}
        other => {
            eprintln!("web_demo: --cli-override: unknown path \"{other}\"");
            std::process::exit(2);
        }
    }
}

/// The `--cli-override` table: a `path` without `=value` takes the "no value" value,
/// `path=value` is parsed. `node.name` is not in this table — it has no slot in `CliOverrides`,
/// it is edited directly in the document at step 3.
pub(crate) fn parse_cli_overrides(entries: &[String]) -> CliOverrides {
    let mut cli = CliOverrides::default();
    for entry in entries {
        let (path, value) = split_override_entry(entry);
        apply_cli_override(&mut cli, path, value);
    }
    cli
}
