use super::super::{load_net_file, LoadedNetFile, ParsedNetFile};
use super::*;
use crate::config::config_file::test_support::{
    all_changed, clear_readonly, names_in, scratch, test_secret,
};
use crate::config::config_file::{NetSettings, SaveError};
use std::path::{Path, PathBuf};

const NOW: u64 = 1_790_278_867;
const CREATED: u64 = 1_790_000_000;

mod new_slot {
    use super::*;

    #[test]
    fn gets_revision_one() {
        // given: neither `nets/` nor the slot folder exist
        let setup = setup();
        // when: the file is saved with no expected revision
        let rev = save(&setup.file, None, &NetSettings::default()).expect("save");
        // then: the first revision, and the file reads back with the given creation time
        assert_eq!(rev, 1);
        let p = parsed_net(&setup.file);
        assert_eq!((p.rev, p.created_unix), (1, CREATED));
        setup.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn creates_private_folders_and_a_private_file() {
        // given: neither `nets/` nor the slot folder exist
        let setup = setup();
        // when: the file is saved
        save(&setup.file, None, &NetSettings::default()).expect("save");
        // then: both folders are 0700 and the file is 0600
        assert_eq!(mode_of(&setup.nets_dir()), 0o700, "nets/");
        assert_eq!(mode_of(&setup.slot_dir()), 0o700, "nets/<id>/");
        assert_eq!(mode_of(&setup.file), 0o600, "config.json");
        setup.cleanup();
    }

    #[test]
    fn leaves_no_temporary_file() {
        // given: neither `nets/` nor the slot folder exist
        let setup = setup();
        // when: the file is saved
        save(&setup.file, None, &NetSettings::default()).expect("save");
        // then: the slot folder holds the file alone
        assert_eq!(names_in(&setup.slot_dir()), ["config.json"]);
        setup.cleanup();
    }

    #[test]
    fn keeps_the_net_settings() {
        // given: net settings where every field differs from the default
        let setup = setup();
        let net = all_changed().net;
        // when: saved and read back
        save(&setup.file, None, &net).expect("save");
        // then: the same settings, with nothing reported
        let p = parsed_net(&setup.file);
        assert_eq!(p.net, net);
        assert!(p.field_errors.is_empty(), "{:?}", p.field_errors);
        setup.cleanup();
    }
}

mod existing_slot {
    use super::*;

    #[test]
    fn bumps_the_revision_when_it_matches() {
        // given: a slot file at revision 1
        let setup = setup();
        save(&setup.file, None, &NetSettings::default()).expect("first save");
        // when: saved again with the revision that was read
        let rev = save(&setup.file, Some(1), &NetSettings::default()).expect("second save");
        // then: the revision grew
        assert_eq!(rev, 2);
        assert_eq!(parsed_net(&setup.file).rev, 2);
        setup.cleanup();
    }

    #[test]
    fn keeps_the_creation_time_it_is_given() {
        // given: a slot file written with one creation time
        let setup = setup();
        save(&setup.file, None, &NetSettings::default()).expect("first save");
        // when: rewritten with the same creation time later
        let again = save_net_file(
            &setup.file,
            Some(1),
            NOW + 60,
            CREATED,
            &test_secret(),
            &NetSettings::default(),
        );
        // then: the creation time is unchanged
        again.expect("second save");
        assert_eq!(parsed_net(&setup.file).created_unix, CREATED);
        setup.cleanup();
    }
}

mod stale_revision {
    use super::*;

    #[test]
    fn is_refused_and_the_file_stays() {
        // given: a file at revision 5
        let setup = setup();
        save_up_to_rev(&setup.file, 5);
        let before = std::fs::read(&setup.file).expect("reading before");
        // when: saved with expected revision 3
        let got = save(&setup.file, Some(3), &all_changed().net);
        // then: refused with the revision on disk, the file is byte for byte the same
        assert!(matches!(got, Err(SaveError::StaleRev(5))), "got {got:?}");
        assert_eq!(std::fs::read(&setup.file).expect("reading after"), before);
        assert_eq!(names_in(&setup.slot_dir()), ["config.json"]);
        setup.cleanup();
    }

    #[test]
    fn vanished_file_reports_revision_zero() {
        // given: no file on disk
        let setup = setup();
        // when: saved with an expected revision
        let got = save(&setup.file, Some(1), &NetSettings::default());
        // then: refused, and nothing is created
        assert!(matches!(got, Err(SaveError::StaleRev(0))), "got {got:?}");
        assert!(!setup.nets_dir().exists(), "nets/ was created");
        setup.cleanup();
    }
}

mod read_only_file {
    use super::*;

    #[test]
    fn is_refused_without_a_temporary_file() {
        // given: a slot file marked read-only
        let setup = setup();
        save(&setup.file, None, &NetSettings::default()).expect("first save");
        let mut perms = std::fs::metadata(&setup.file)
            .expect("metadata")
            .permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&setup.file, perms).expect("marking read-only");
        let before = std::fs::read(&setup.file).expect("reading before");
        // when: saved
        let got = save(&setup.file, Some(1), &all_changed().net);
        // then: an `Io` refusal, the file is the same and no temporary file appeared
        clear_readonly(&setup.file);
        assert!(matches!(got, Err(SaveError::Io(_))), "got {got:?}");
        assert_eq!(std::fs::read(&setup.file).expect("reading after"), before);
        assert_eq!(names_in(&setup.slot_dir()), ["config.json"]);
        setup.cleanup();
    }
}

struct Setup {
    root: PathBuf,
    file: PathBuf,
}

impl Setup {
    fn nets_dir(&self) -> PathBuf {
        self.root.join("nets")
    }

    fn slot_dir(&self) -> PathBuf {
        self.nets_dir().join("ab2c3d")
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A fresh settings folder with no `nets/` in it; `file` is where the slot file goes.
fn setup() -> Setup {
    let root = scratch("net-save");
    let file = root.join("nets").join("ab2c3d").join("config.json");
    Setup { root, file }
}

fn save(path: &Path, expected_rev: Option<u64>, net: &NetSettings) -> Result<u64, SaveError> {
    save_net_file(path, expected_rev, NOW, CREATED, &test_secret(), net)
}

/// Saves the same file until its revision is `rev`.
fn save_up_to_rev(path: &Path, rev: u64) {
    let net = NetSettings::default();
    save(path, None, &net).expect("first save");
    for current in 1..rev {
        save(path, Some(current), &net).expect("later save");
    }
}

fn parsed_net(path: &Path) -> ParsedNetFile {
    match load_net_file(path) {
        LoadedNetFile::Parsed(p) => p,
        other => panic!("expected Parsed, got {other:?}"),
    }
}

#[cfg(unix)]
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}
