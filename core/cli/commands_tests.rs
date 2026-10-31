use super::*;
use crate::config::config_file::PathRule;
use crate::members::network_key::managed_key_with_label;
use crate::transport::net::NodeSecret;

fn overrides_with_key(key: &str) -> CliOverrides {
    CliOverrides {
        network_key: Some(key.to_string()),
        ..CliOverrides::default()
    }
}

#[test]
fn an_unreadable_key_is_not_given_to_the_main_network() {
    let main = overrides_for_main(&overrides_with_key("garbage"), &KeyNet::Unreadable);
    assert_eq!(main.network_key, None);
}

#[test]
fn the_main_network_keeps_the_cli_key() {
    let main = overrides_for_main(&overrides_with_key("garbage"), &KeyNet::Main);
    assert_eq!(main.network_key.as_deref(), Some("garbage"));
}

fn key_of(seed: u8) -> String {
    let creator = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    managed_key_with_label(None, &[seed; 32], &creator)
}

/// A main network with the key `seed` in a fresh settings file; removed with the returned directory.
fn main_run(name: &str, seed: u8) -> (std::path::PathBuf, config_file::RunConfig) {
    let dir = std::env::temp_dir().join(format!("commands_tests-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &overrides_with_key(&key_of(seed)), true, 0)
        .expect("prepare");
    (dir, run)
}

mod adding_a_key_network {
    use super::*;

    #[test]
    fn saves_a_new_network_as_a_slot() {
        let (dir, run) = main_run("add", 9);
        let other = key_of(8);
        let added = add_key_net(&KeyNet::Add, Some(&other), &run, &[], 5).expect("add");
        assert!(added);
        let saved = run_invite_addnet::saved_slot_keys(&dir.join("config.json"));
        assert_eq!(saved, vec![Some(other)]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn leaves_an_unreadable_key_unsaved_but_owned_by_a_slot() {
        let (dir, run) = main_run("unreadable", 9);
        let added = add_key_net(&KeyNet::Unreadable, Some("garbage"), &run, &[], 5).expect("ok");
        assert!(added);
        assert!(run_invite_addnet::saved_slot_keys(&dir.join("config.json")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn does_not_claim_the_key_of_the_main_network() {
        let (dir, run) = main_run("main", 9);
        let added = add_key_net(&KeyNet::Main, Some(&key_of(9)), &run, &[], 5).expect("ok");
        assert!(!added);
        assert!(run_invite_addnet::saved_slot_keys(&dir.join("config.json")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fails_when_there_is_no_settings_file() {
        let (dir, mut run) = main_run("nofile", 9);
        run.file_report.path = None;
        let err = add_key_net(&KeyNet::Add, Some(&key_of(8)), &run, &[], 5);
        assert!(err.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn set_flags_reach_overrides() {
    use clap::FromArgMatches;
    let all = [
        "meshtown",
        "run",
        "--set",
        "probe.interval_sec=20",
        "--set",
        "checks.enabled=false",
    ];
    let m = crate::cli::args::Cli::command()
        .try_get_matches_from(all)
        .unwrap();
    let sub = m.subcommand_matches("run").unwrap();
    let crate::cli::args::CliCommand::Run(args) =
        crate::cli::args::Cli::from_arg_matches(&m).unwrap().command
    else {
        panic!("expected run");
    };
    let overrides = build_overrides(sub, &args).unwrap();
    let mut s = config_file::Settings::default();
    let paths = overrides.apply(&mut s);
    assert_eq!(paths, vec!["probe.interval_sec", "checks.enabled"]);
    assert_eq!(s.net.probe_interval_sec, 20);
    assert!(!s.net.checks_enabled);
}
