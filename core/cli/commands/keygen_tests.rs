use super::*;
use crate::config::config_file::{CliOverrides, ConfigSource, PathRule, Settings};
use crate::members::network_key::parse_user_input;
use crate::transport::net::NodeSecret;

/// A directory for the settings file; removed in `Drop`.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let dir = std::env::temp_dir().join(format!("keygen_tests-{name}-{}", std::process::id()));
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

fn write_file_with_secret(path: &Path, seed: u8) -> PeerId {
    let secret = NodeSecret::from_bytes(&[seed; 32]);
    config_file::save_config_file(path, None, 0, &secret, &Settings::default()).expect("save");
    secret.peer_id()
}

mod with_an_identity_in_the_file {
    use super::*;

    #[test]
    fn the_key_is_rooted_at_it() {
        // given: a file with the secret [9; 32]
        let dir = Dir::new("rooted");
        let me = write_file_with_secret(&dir.file(), 9);
        let before = std::fs::read(dir.file()).unwrap();
        // when: the key is made
        let root = node_identity(Some(&dir.file()), 5).expect("identity");
        let text = new_key(&["our".into(), "game".into()], false, &root).expect("key");
        // then: its root is the node, and the file is byte for byte as it was
        assert_eq!(root, me);
        let parsed = parse_user_input(&text).expect("parses");
        assert_eq!(parsed.root, me);
        assert_eq!(parsed.label.as_deref(), Some("our-game"));
        assert_eq!(std::fs::read(dir.file()).unwrap(), before);
    }

    mod raw {
        use super::*;

        #[test]
        fn is_the_mesh2_form() {
            // given
            let dir = Dir::new("raw");
            let me = write_file_with_secret(&dir.file(), 9);
            // when
            let root = node_identity(Some(&dir.file()), 5).expect("identity");
            let text = new_key(&[], true, &root).expect("key");
            // then
            assert!(text.starts_with("mesh2-"), "{text}");
            assert_eq!(parse_user_input(&text).expect("parses").root, me);
        }
    }
}

mod without_a_file {
    use super::*;

    #[test]
    fn creates_it_and_run_keeps_the_identity() {
        // given: no file
        let dir = Dir::new("fresh");
        assert!(!dir.file().exists());
        // when: keygen takes the identity, then the first run prepares the same file
        let root = node_identity(Some(&dir.file()), 5).expect("identity");
        let source = ConfigSource::File {
            path: dir.file(),
            may_create: true,
            rule: PathRule::Explicit,
        };
        let run = config_file::prepare(&source, &CliOverrides::default(), true, 6).expect("run");
        // then: the file exists and run has the identity keygen used
        assert!(dir.file().exists());
        assert_eq!(run.secret.peer_id(), root);
    }
}

mod with_a_file_without_identity {
    use super::*;

    #[test]
    fn refuses_and_leaves_the_file() {
        // given: a valid file whose envelope has no identity
        let dir = Dir::new("noid");
        write_file_with_secret(&dir.file(), 9);
        let mut doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.file()).unwrap()).unwrap();
        doc.as_object_mut().unwrap().remove("identity");
        std::fs::write(dir.file(), serde_json::to_vec(&doc).unwrap()).unwrap();
        let before = std::fs::read(dir.file()).unwrap();
        // when
        let err = node_identity(Some(&dir.file()), 5).expect_err("refused");
        // then
        assert!(
            format!("{err:#}").contains("has no identity for this computer yet"),
            "{err:#}"
        );
        assert_eq!(std::fs::read(dir.file()).unwrap(), before);
    }
}

mod with_an_invalid_file {
    use super::*;

    #[test]
    fn refuses_as_unusable() {
        // given: a file that is not a document
        let dir = Dir::new("invalid");
        std::fs::write(dir.file(), b"not json").unwrap();
        // when
        let err = node_identity(Some(&dir.file()), 5).expect_err("refused");
        // then
        assert!(
            format!("{err:#}").starts_with("settings file is unusable: "),
            "{err:#}"
        );
        assert_eq!(std::fs::read(dir.file()).unwrap(), b"not json");
    }
}
