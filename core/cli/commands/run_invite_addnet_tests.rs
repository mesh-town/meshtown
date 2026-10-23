use super::*;
use crate::config::config_file::{CliOverrides, PathRule, SecretRead};
use crate::members::network_key::NetworkKey;
use crate::members::sync::replica::file::Bak;
use std::path::PathBuf;

fn key_a() -> String {
    NetworkKey::from_bytes([9u8; 32]).encode()
}

fn key_b() -> String {
    NetworkKey::from_bytes([8u8; 32]).encode()
}

fn key_c() -> String {
    NetworkKey::from_bytes([7u8; 32]).encode()
}

mod planning_a_key {
    use super::*;

    #[test]
    fn uses_the_main_network_when_the_file_has_no_key() {
        let out = plan_key_net(Some(&key_b()), None, &[]);
        assert_eq!(out, KeyNet::Main);
    }

    #[test]
    fn uses_the_main_network_for_the_key_of_the_file() {
        let out = plan_key_net(Some(&key_a()), Some(&key_a()), &[]);
        assert_eq!(out, KeyNet::Main);
    }

    #[test]
    fn finds_the_network_that_has_a_slot() {
        let slots = vec![None, Some(key_b())];
        let out = plan_key_net(Some(&key_b()), Some(&key_a()), &slots);
        assert_eq!(out, KeyNet::Existing);
    }

    #[test]
    fn adds_a_network_without_a_slot() {
        let slots = vec![Some(key_c())];
        let out = plan_key_net(Some(&key_b()), Some(&key_a()), &slots);
        assert_eq!(out, KeyNet::Add);
    }

    #[test]
    fn refuses_a_network_over_the_limit() {
        let slots = vec![Some(key_c()); MAX_NETS - 1];
        let out = plan_key_net(Some(&key_b()), Some(&key_a()), &slots);
        assert_eq!(out, KeyNet::TooMany);
    }

    #[test]
    fn has_nothing_to_decide_without_a_key() {
        assert_eq!(plan_key_net(None, Some(&key_a()), &[]), KeyNet::Main);
    }
}

/// A directory for the settings file; removed in `Drop`.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let dir =
            std::env::temp_dir().join(format!("run_invite_addnet-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }

    fn file(&self) -> PathBuf {
        self.0.join("config.json")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn source(dir: &Dir) -> ConfigSource {
    ConfigSource::File {
        path: dir.file(),
        may_create: true,
        rule: PathRule::Explicit,
    }
}

fn main_run(dir: &Dir, key: &str) -> RunConfig {
    let cli = CliOverrides {
        network_key: Some(key.to_string()),
        ..CliOverrides::default()
    };
    config_file::prepare(&source(dir), &cli, true, 0).expect("prepare")
}

mod adding_a_slot {
    use super::*;

    #[test]
    fn adds_a_network_slot_and_keeps_the_main_file() {
        // given
        let dir = Dir::new("adds");
        let run = main_run(&dir, &key_a());
        let before = std::fs::read(dir.file()).unwrap();
        // when
        let added = add_net_slot(&run, &key_b(), &[], 5).expect("add");
        // then
        let path = net_slot_path(&dir.file(), &added.id).unwrap();
        assert_eq!(slot_key(&path), Some(key_b()));
        assert_eq!(std::fs::read(dir.file()).unwrap(), before);
        assert_eq!(saved_slot_keys(&dir.file()), vec![Some(key_b())]);
    }

    #[test]
    fn gives_the_slot_its_own_secret() {
        let dir = Dir::new("secret");
        let run = main_run(&dir, &key_a());
        let added = add_net_slot(&run, &key_b(), &[], 5).expect("add");
        let path = net_slot_path(&dir.file(), &added.id).unwrap();
        let LoadedNetFile::Parsed(p) = load_net_file(&path) else {
            panic!("the slot file must parse");
        };
        let (SecretRead::Valid(slot), SecretRead::Valid(main)) =
            (p.secret, config_file_secret(&dir))
        else {
            panic!("both files must hold a valid secret");
        };
        assert_ne!(slot.to_bytes(), main.to_bytes());
    }

    #[test]
    fn refuses_without_a_settings_file() {
        let mut run = main_run(&Dir::new("nofile"), &key_a());
        run.file_report.path = None;
        assert!(add_net_slot(&run, &key_b(), &[], 5).is_err());
    }

    #[test]
    fn peeks_at_the_file_key_without_writing() {
        let dir = Dir::new("peek");
        assert_eq!(peek_file_key(&dir.file()), None);
        main_run(&dir, &key_a());
        assert_eq!(peek_file_key(&dir.file()), Some(key_a()));
    }

    #[test]
    fn plans_from_the_files_on_disk() {
        let dir = Dir::new("plan");
        let run = main_run(&dir, &key_a());
        add_net_slot(&run, &key_b(), &[], 5).expect("add");
        let src = source(&dir);
        assert_eq!(
            plan_cli_key(&src, Some(&key_b())).unwrap(),
            KeyNet::Existing
        );
        assert_eq!(plan_cli_key(&src, Some(&key_c())).unwrap(), KeyNet::Add);
        assert_eq!(
            plan_cli_key(&ConfigSource::Disabled, Some(&key_c())).unwrap(),
            KeyNet::Main
        );
    }
}

fn config_file_secret(dir: &Dir) -> SecretRead {
    match config_file::load_config_file(&dir.file()) {
        LoadedConfigFile::Parsed(p) => p.secret,
        _ => panic!("the settings file must parse"),
    }
}

fn entry(domain: &str, value: &[u8], ts: u64) -> WireEntry {
    WireEntry {
        author: [1; 32],
        key: format!("list/{domain}/").into_bytes(),
        value: value.to_vec(),
        ts_micros: ts,
        sig: [[0; 32]; 2],
    }
}

fn write_domains(dir: &Dir, entries: &[WireEntry]) {
    let id = crate::members::sync::replica::ids::domains_id();
    let folder = dir.0.join("replica");
    std::fs::create_dir_all(&folder).unwrap();
    let bytes = replica_file::encode(&id, None, entries);
    replica_file::save(
        &replica_file::path_for(&folder, "domains", &id),
        &bytes,
        Bak::Keep,
    )
    .unwrap();
}

mod reading_the_site_list {
    use super::*;

    #[test]
    fn keeps_live_sites_and_drops_tombstones() {
        let dir = Dir::new("domains");
        write_domains(
            &dir,
            &[
                entry("a.example", b"x", 1),
                entry("b.example", b"x", 1),
                entry("b.example", b"", 2),
                entry("c.example", b"x", 3),
            ],
        );
        assert_eq!(
            read_main_domains(&dir.file()),
            vec!["a.example", "c.example"]
        );
    }

    #[test]
    fn is_empty_without_the_file() {
        assert!(read_main_domains(&Dir::new("nodomains").file()).is_empty());
    }

    #[test]
    fn is_empty_for_a_damaged_file() {
        let dir = Dir::new("brokendomains");
        let folder = dir.0.join("replica");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("domains.bin"), b"not a replica file").unwrap();
        assert!(read_main_domains(&dir.file()).is_empty());
    }

    #[test]
    fn seeds_a_new_slot_with_the_list_and_typed_sites_without_repeats() {
        let dir = Dir::new("seed");
        let run = main_run(&dir, &key_a());
        write_domains(&dir, &[entry("a.example", b"x", 1)]);
        let typed = vec!["a.example".to_string(), "d.example".to_string()];
        let added = add_net_slot(&run, &key_b(), &typed, 5).expect("add");
        assert_eq!(added.seed_domains, vec!["a.example", "d.example"]);
    }
}
