use super::*;
use crate::config::config_file::{self, ConfigSource, PathRule};
use crate::members::network_key::test_net;
use crate::members::network_key::LEGACY_KEY_TEXT;
use std::path::PathBuf;

const FALLBACK_TAIL: &str = "; starting without a network";

fn old_key() -> String {
    format!("mesh1-{}", "a".repeat(52))
}

/// A directory for the settings file; removed in `Drop`.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let dir = std::env::temp_dir().join(format!("run_key_tests-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_from(source: &ConfigSource) -> RunConfig {
    config_file::prepare(source, &CliOverrides::default(), true, 0).expect("prepare")
}

fn run_with_file(dir: &Dir) -> RunConfig {
    run_from(&ConfigSource::File {
        path: dir.0.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    })
}

fn journal_has(needle: &str) -> bool {
    let page = crate::node::output::log_page(0, 10_000);
    page.lines.iter().any(|l| l.text.contains(needle))
}

mod main_key {
    use super::*;

    #[test]
    fn an_old_key_with_a_file_falls_back() {
        // given: a run with a settings file
        let dir = Dir::new("old-file");
        let run = run_with_file(&dir);
        // when: the key is an old one
        let out = main_key(&old_key(), &run).expect("no error with a file");
        // then: no key, and the journal says why
        assert!(out.is_none());
        let line = format!("network key did not parse: {LEGACY_KEY_TEXT}{FALLBACK_TAIL}");
        assert!(journal_has(&line), "missing: {line}");
    }

    #[test]
    fn an_old_key_without_a_file_is_an_error() {
        // given: a run without a settings file
        let run = run_from(&ConfigSource::Disabled);
        // when
        let err = main_key(&old_key(), &run).expect_err("an error");
        // then: the reason is the outdated-key text, with no fallback line
        assert_eq!(format!("{err:#}"), LEGACY_KEY_TEXT);
        assert!(!journal_has(&format!("{LEGACY_KEY_TEXT}{FALLBACK_TAIL}")));
    }

    #[test]
    fn a_good_key_passes_root_and_label() {
        // given
        let net = test_net("Office");
        let run = run_from(&ConfigSource::Disabled);
        // when
        let out = main_key(&net.text, &run).expect("parses");
        // then
        let input = out.expect("a key");
        assert_eq!(input.root, net.root);
        assert_eq!(input.label.as_deref(), Some("office"));
    }
}
