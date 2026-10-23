use crate::config::config_file::net_file::render_net_file;
use crate::config::config_file::slot_store::ids::new_slot_id_with;
use crate::config::config_file::slot_store::{
    list_net_slots, new_slot_id, set_aside_main, set_aside_slot, NetSlotId, NetSlotOnDisk,
};
use crate::config::config_file::test_support::{names_in, sample_settings, scratch, test_secret};
use crate::config::config_file::{
    render_config_file, stamp_utc, LoadedNetFile, NetSettings, FILE_NAME,
};
use std::path::PathBuf;

const NOW: u64 = 1_790_278_867;

mod listing {
    use super::*;

    #[test]
    fn without_a_nets_folder_gives_nothing_and_creates_nothing() {
        // given: a settings folder with only the main file
        let dir = setup();
        // when: the slots are listed
        let slots = list_net_slots(&dir.main);
        // then: no slots, and no `nets/` appeared
        assert!(slots.is_empty());
        assert!(!dir.nets().exists(), "`nets/` was created");
        dir.cleanup();
    }

    #[test]
    fn skips_set_aside_folders_foreign_names_and_plain_files() {
        // given: one real slot among set-aside folders, foreign names and files
        let dir = setup();
        dir.write_slot("ab2c3d", 10);
        dir.write_slot("left-ab2c3d-20261023T100000Z", 11);
        dir.write_slot("left-main-20261023T100000Z", 12);
        dir.write_slot("ZZZZZZ", 13);
        dir.write_slot("main", 14);
        dir.write_slot("toolongname", 15);
        std::fs::write(dir.nets().join("x"), "not a folder").expect("writing a file");
        std::fs::create_dir(dir.nets().join("efgh23")).expect("a folder with no file");
        // when: the slots are listed
        let ids = ids_of(&list_net_slots(&dir.main));
        // then: only the real slot is there
        assert_eq!(ids, ["ab2c3d"]);
        dir.cleanup();
    }

    #[test]
    fn puts_parsed_slots_in_creation_order() {
        // given: two slots created at 20 and 10, ids in the opposite order
        let dir = setup();
        dir.write_slot("bbbbbb", 20);
        dir.write_slot("dddddd", 10);
        // when: the slots are listed
        let ids = ids_of(&list_net_slots(&dir.main));
        // then: the older one is first
        assert_eq!(ids, ["dddddd", "bbbbbb"]);
        dir.cleanup();
    }

    #[test]
    fn breaks_a_creation_time_tie_by_id() {
        // given: two slots created at the same time
        let dir = setup();
        dir.write_slot("cccccc", 10);
        dir.write_slot("bbbbbb", 10);
        // when: the slots are listed
        let ids = ids_of(&list_net_slots(&dir.main));
        // then: the smaller id is first
        assert_eq!(ids, ["bbbbbb", "cccccc"]);
        dir.cleanup();
    }

    #[test]
    fn puts_broken_files_last_by_id() {
        // given: two broken slots with ids before and after the parsed one
        let dir = setup();
        dir.write_raw("ffffff", "{ not json");
        dir.write_slot("cccccc", 99);
        dir.write_raw("aaaaaa", "{ not json");
        // when: the slots are listed
        let slots = list_net_slots(&dir.main);
        // then: the parsed one leads, the broken ones follow by id and say why
        assert_eq!(ids_of(&slots), ["cccccc", "aaaaaa", "ffffff"]);
        assert!(matches!(slots[1].file, LoadedNetFile::Invalid(_)));
        dir.cleanup();
    }

    #[test]
    fn names_the_file_of_each_slot() {
        // given: one slot
        let dir = setup();
        dir.write_slot("ab2c3d", 10);
        // when: the slots are listed
        let slots = list_net_slots(&dir.main);
        // then: the path is `nets/<id>/config.json` and the file is parsed
        assert_eq!(slots[0].path, dir.nets().join("ab2c3d").join(FILE_NAME));
        assert!(matches!(slots[0].file, LoadedNetFile::Parsed(_)));
        dir.cleanup();
    }
}

mod new_id {
    use super::*;

    #[test]
    fn skips_an_id_that_is_taken() {
        // given: `nets/ab2c3d` exists; the generator gives it first, then a free one
        let dir = setup();
        dir.write_slot("ab2c3d", 10);
        let mut next = feed(&["ab2c3d", "xyz234"]);
        // when: an id is picked
        let id = new_slot_id_with(&dir.main, &mut next).expect("a free id");
        // then: the free one
        assert_eq!(id.as_str(), "xyz234");
        dir.cleanup();
    }

    #[test]
    fn skips_an_id_held_by_a_set_aside_folder() {
        // given: `nets/left-ab2c3d-<stamp>` exists; the generator gives that id first
        let dir = setup();
        dir.write_slot("left-ab2c3d-20261023T100000Z", 10);
        let mut next = feed(&["ab2c3d", "xyz234"]);
        // when: an id is picked
        let id = new_slot_id_with(&dir.main, &mut next).expect("a free id");
        // then: the id of the set-aside folder is not reused
        assert_eq!(id.as_str(), "xyz234");
        dir.cleanup();
    }

    #[test]
    fn gives_up_after_sixteen_taken_ids() {
        // given: a generator that only repeats a taken id
        let dir = setup();
        dir.write_slot("ab2c3d", 10);
        let mut calls = 0;
        let mut next = || {
            calls += 1;
            "ab2c3d".to_string()
        };
        // when: an id is picked
        let got = new_slot_id_with(&dir.main, &mut next);
        // then: an error after sixteen attempts
        assert!(got.is_err());
        assert_eq!(calls, 16);
        dir.cleanup();
    }

    #[test]
    fn random_id_is_a_valid_free_slot_name() {
        // given: a settings folder with one slot
        let dir = setup();
        dir.write_slot("ab2c3d", 10);
        // when: an id is picked at random
        let id = new_slot_id(&dir.main).expect("an id");
        // then: it parses as a slot id, is not `main` and is not taken
        assert_eq!(NetSlotId::parse(id.as_str()).as_ref(), Some(&id));
        assert_ne!(id, NetSlotId::main());
        assert!(!dir.nets().join(id.as_str()).exists());
        dir.cleanup();
    }
}

mod setting_a_slot_aside {
    use super::*;

    #[test]
    fn moves_the_folder_to_a_left_name() {
        // given: a slot folder
        let dir = setup();
        dir.write_slot("ab2c3d", 10);
        let before = std::fs::read(dir.slot_file("ab2c3d")).expect("reading before");
        // when: it is set aside
        let target = set_aside_slot(&dir.main, &slot_id("ab2c3d"), NOW).expect("set aside");
        // then: the folder is `left-<id>-<stamp>` with the same file, and the slot is gone
        let name = format!("left-ab2c3d-{}", stamp_utc(NOW));
        assert_eq!(target, dir.nets().join(&name));
        assert_eq!(
            std::fs::read(target.join(FILE_NAME)).expect("moved file"),
            before
        );
        assert!(list_net_slots(&dir.main).is_empty());
        dir.cleanup();
    }

    #[test]
    fn refuses_when_the_target_name_is_taken() {
        // given: a slot folder and a folder already holding the target name
        let dir = setup();
        dir.write_slot("ab2c3d", 10);
        dir.write_slot(&format!("left-ab2c3d-{}", stamp_utc(NOW)), 5);
        // when: the slot is set aside
        let got = set_aside_slot(&dir.main, &slot_id("ab2c3d"), NOW);
        // then: an error and the slot folder is intact
        assert!(got.is_err());
        assert!(dir.slot_file("ab2c3d").is_file());
        dir.cleanup();
    }

    #[test]
    fn refuses_the_main_network_and_a_missing_folder() {
        // given: a settings folder with no slots
        let dir = setup();
        std::fs::create_dir_all(dir.nets()).expect("nets");
        // when: `main` and an id with no folder are set aside
        let main = set_aside_slot(&dir.main, &NetSlotId::main(), NOW);
        let missing = set_aside_slot(&dir.main, &slot_id("ab2c3d"), NOW);
        // then: both are errors and nothing was created
        assert!(main.is_err() && missing.is_err());
        assert!(names_in(&dir.nets()).is_empty());
        dir.cleanup();
    }
}

mod setting_the_main_network_aside {
    use super::*;

    #[test]
    fn copies_the_main_file_and_moves_the_caches() {
        // given: the main file and its four caches
        let dir = setup_with_caches();
        let main_before = std::fs::read(&dir.main).expect("reading the main file");
        // when: the main network is set aside
        let target = set_aside_main(&dir.main, NOW).expect("set aside");
        // then: the folder holds a byte-for-byte copy and all caches; the root holds no caches
        assert_eq!(
            target,
            dir.nets().join(format!("left-main-{}", stamp_utc(NOW)))
        );
        assert_eq!(
            std::fs::read(target.join(FILE_NAME)).expect("copy"),
            main_before
        );
        assert_eq!(
            names_in(&target),
            [
                "config.json",
                "docs-store",
                "peer-names.json",
                "peers-cache.json",
                "replica"
            ]
        );
        assert_eq!(names_in(&dir.root), ["config.json", "nets"]);
        assert_eq!(std::fs::read(&dir.main).expect("main after"), main_before);
        dir.cleanup();
    }

    #[test]
    fn without_caches_copies_the_file_alone() {
        // given: only the main file
        let dir = setup();
        // when: the main network is set aside
        let target = set_aside_main(&dir.main, NOW).expect("set aside");
        // then: the folder holds the copy and nothing else
        assert_eq!(names_in(&target), ["config.json"]);
        dir.cleanup();
    }

    #[test]
    fn refuses_a_taken_target_and_leaves_everything() {
        // given: the main file with caches, and the target folder already there
        let dir = setup_with_caches();
        let target = dir.nets().join(format!("left-main-{}", stamp_utc(NOW)));
        std::fs::create_dir_all(&target).expect("the taken target");
        // when: the main network is set aside
        let got = set_aside_main(&dir.main, NOW);
        // then: an error, the target is still empty and the caches stay beside the main file
        assert!(got.is_err());
        assert!(names_in(&target).is_empty());
        assert_eq!(
            names_in(&dir.root),
            [
                "config.json",
                "docs-store",
                "nets",
                "peer-names.json",
                "peers-cache.json",
                "replica"
            ]
        );
        dir.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn keeps_the_folder_private() {
        // given: the main file
        let dir = setup();
        // when: the main network is set aside
        let target = set_aside_main(&dir.main, NOW).expect("set aside");
        // then: the folder is 0700 and the copy is 0600
        assert_eq!(mode_of(&target), 0o700);
        assert_eq!(mode_of(&target.join(FILE_NAME)), 0o600);
        dir.cleanup();
    }

    #[test]
    fn leaves_nothing_to_list() {
        // given: a main network that was set aside
        let dir = setup_with_caches();
        set_aside_main(&dir.main, NOW).expect("set aside");
        // when: the slots are listed; then: the `left-main-*` folder is not a slot
        assert!(list_net_slots(&dir.main).is_empty());
        dir.cleanup();
    }
}

struct Dir {
    root: PathBuf,
    main: PathBuf,
}

impl Dir {
    fn nets(&self) -> PathBuf {
        self.root.join("nets")
    }

    fn slot_file(&self, name: &str) -> PathBuf {
        self.nets().join(name).join(FILE_NAME)
    }

    /// A valid slot file created at `created_unix`, in the folder `name`.
    fn write_slot(&self, name: &str, created_unix: u64) {
        let key = "label--test-key-not-real".to_string();
        let net = NetSettings {
            network_key: Some(key),
            ..NetSettings::default()
        };
        let text = render_net_file(1, NOW, created_unix, &test_secret(), &net);
        self.write_raw(name, &text);
    }

    fn write_raw(&self, name: &str, text: &str) {
        std::fs::create_dir_all(self.nets().join(name)).expect("slot folder");
        std::fs::write(self.slot_file(name), text).expect("slot file");
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A settings folder holding only the main file.
fn setup() -> Dir {
    let root = scratch("slot-list");
    let main = root.join(FILE_NAME);
    let text = render_config_file(1, NOW, &test_secret(), &sample_settings());
    std::fs::write(&main, text).expect("main file");
    Dir { root, main }
}

/// The main file plus the four caches that sit beside it.
fn setup_with_caches() -> Dir {
    let dir = setup();
    for file in ["peers-cache.json", "peer-names.json"] {
        std::fs::write(dir.root.join(file), "{}").expect("cache file");
    }
    for folder in ["docs-store", "replica"] {
        std::fs::create_dir(dir.root.join(folder)).expect("cache folder");
        std::fs::write(dir.root.join(folder).join("part"), "x").expect("cache part");
    }
    dir
}

fn ids_of(slots: &[NetSlotOnDisk]) -> Vec<String> {
    slots.iter().map(|s| s.id.to_string()).collect()
}

fn slot_id(text: &str) -> NetSlotId {
    NetSlotId::parse(text).expect("a valid slot id")
}

/// A generator that answers with the given texts in turn.
fn feed(texts: &[&str]) -> impl FnMut() -> String {
    let mut left: Vec<String> = texts.iter().rev().map(|t| t.to_string()).collect();
    move || left.pop().expect("the generator ran dry")
}

#[cfg(unix)]
fn mode_of(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}
