use super::{copy_shared_net, fresh_net_slot};
use crate::config::config_file::test_support::{scratch, test_secret};
use crate::config::config_file::{
    prepare, save_config_file, CliOverrides, ConfigSource, NetSettings, NetSlotId, PathRule,
    RunConfig, Settings, FILE_NAME,
};

const NOW: u64 = 1_790_278_867;

fn main_run() -> (std::path::PathBuf, RunConfig) {
    let root = scratch("slot-fresh");
    let file = root.join(FILE_NAME);
    save_config_file(&file, None, 1, &test_secret(), &Settings::default()).expect("main file");
    let source = ConfigSource::File {
        path: file,
        may_create: true,
        rule: PathRule::Explicit,
    };
    let cli = CliOverrides {
        socks_port: Some(2000),
        ..CliOverrides::default()
    };
    (root, prepare(&source, &cli, false, NOW).expect("main run"))
}

fn changed() -> NetSettings {
    NetSettings {
        probe_interval_sec: 120,
        checks_enabled: false,
        check_interval_sec: 61,
        doh_server: "9.9.9.9:443".parse().unwrap(),
        provider_dns: Some("10.0.0.1:53".parse().unwrap()),
        exit_enabled: !NetSettings::default().exit_enabled,
        node_name: Some("laptop".into()),
        network_key: Some("a-key".into()),
        ..NetSettings::default()
    }
}

#[test]
fn copying_moves_the_five_shared_fields_and_nothing_else() {
    let from = changed();
    let mut to = NetSettings::default();
    copy_shared_net(&from, &mut to);
    assert_eq!(to.probe_interval_sec, 120);
    assert!(!to.checks_enabled);
    assert_eq!(to.check_interval_sec, 61);
    assert_eq!(to.doh_server, from.doh_server);
    assert_eq!(to.provider_dns, from.provider_dns);
    let own = NetSettings::default();
    assert_eq!(to.exit_enabled, own.exit_enabled);
    assert_eq!(to.node_name, None);
    assert_eq!(to.network_key, None);
}

#[test]
fn a_fresh_slot_lives_under_nets_with_socks_off_and_no_file_on_disk() {
    let (root, main) = main_run();
    let id = NetSlotId::parse("ab2c3d").unwrap();
    let run = fresh_net_slot(&main, &changed(), &id, NOW).unwrap();
    let path = root.join("nets").join("ab2c3d").join(FILE_NAME);
    assert_eq!(run.file.as_ref().unwrap().path(), path);
    assert_eq!(run.file_report.path.as_deref(), Some(path.as_path()));
    assert!(!path.exists(), "nothing is written");
    let process = &run.effective_settings.process;
    assert_eq!((process.socks_port, process.socks_enabled), (2000, false));
    assert_eq!(run.effective_settings.net.probe_interval_sec, 120);
    assert_eq!(run.file_settings.net.probe_interval_sec, 120);
    assert_eq!(run.effective_settings.net.network_key, None);
    assert_ne!(run.secret.to_bytes(), main.secret.to_bytes());
}
