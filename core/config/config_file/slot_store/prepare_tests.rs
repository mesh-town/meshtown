use crate::config::config_file::slot_store::{prepare_net_slot, NetSlotId, NetSlotOnDisk};
use crate::config::config_file::test_support::{clear_readonly, names_in, scratch, test_secret};
use crate::config::config_file::{
    load_net_file, prepare, save_config_file, stamp_utc, CliOverrides, ConfigSource, LoadedNetFile,
    PathRule, RunConfig, SecretRead, Settings, FILE_NAME,
};
use crate::members::network_key::NetworkKey;
use std::path::PathBuf;

const NOW: u64 = 1_790_278_867;
const CREATED: u64 = 1_792_749_600;
const SECRET_B64: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

mod flags_of_the_main {
    use super::*;

    #[test]
    fn socks_port_passes_to_the_slot_with_socks_off() {
        // given: a main started with `--socks-port 2000 --exit`, and a slot file with exit off
        let fx = setup(CliOverrides {
            socks_port: Some(2000),
            exit_enabled: Some(true),
            ..CliOverrides::default()
        });
        fx.write_slot("ab2c3d", &slot_text(&secret_line(), &good_config(false)));
        // when: the slot is prepared
        let run = fx.prepare("ab2c3d").expect("the slot is prepared");
        // then: the port comes from the main, SOCKS stays off, exit comes from the slot file
        let process = &run.effective_settings.process;
        assert_eq!((process.socks_port, process.socks_enabled), (2000, false));
        assert!(
            !run.effective_settings.net.exit_enabled,
            "`--exit` leaked into the slot"
        );
        assert!(!run.file_settings.net.exit_enabled);
    }

    #[test]
    fn only_process_flag_paths_are_kept() {
        // given: a main started with a process flag and a network flag
        let fx = setup(CliOverrides {
            socks_port: Some(2000),
            exit_enabled: Some(true),
            ..CliOverrides::default()
        });
        fx.write_slot("ab2c3d", &slot_text(&secret_line(), &good_config(false)));
        // when: the slot is prepared
        let run = fx.prepare("ab2c3d").expect("the slot is prepared");
        // then: only the process flag is listed as overridden
        assert_eq!(run.cli_override_paths, vec!["socks.port"]);
    }

    #[test]
    fn file_settings_keep_the_process_block_of_the_main_file() {
        // given: a main started with `--socks-port 2000`
        let fx = setup(CliOverrides {
            socks_port: Some(2000),
            ..CliOverrides::default()
        });
        fx.write_slot("ab2c3d", &slot_text(&secret_line(), &good_config(false)));
        // when: the slot is prepared
        let run = fx.prepare("ab2c3d").expect("the slot is prepared");
        // then: the flag does not appear in what the file holds
        assert_eq!(run.file_settings.process, fx.main.file_settings.process);
        assert_eq!(run.file_settings.process.socks_port, 1080);
    }
}

mod slot_identity {
    use super::*;

    #[test]
    fn valid_secret_and_file_are_taken_as_they_are() {
        // given: a slot file with a valid secret
        let fx = setup(CliOverrides::default());
        fx.write_slot("ab2c3d", &slot_text(&secret_line(), &good_config(false)));
        let before = fx.slot_bytes("ab2c3d");
        // when: the slot is prepared
        let run = fx.prepare("ab2c3d").expect("the slot is prepared");
        // then: its own secret and file, persistent, and the file is untouched
        assert_eq!(run.secret.to_bytes(), test_secret().to_bytes());
        assert!(run.file_report.persistent_identity);
        assert_eq!(
            run.file.as_ref().map(|f| f.path().to_path_buf()),
            Some(fx.slot_file("ab2c3d"))
        );
        assert_eq!(fx.slot_bytes("ab2c3d"), before);
    }

    #[test]
    fn missing_secret_is_made_and_written_to_the_slot_file_only() {
        // given: a slot file without a secret
        let fx = setup(CliOverrides::default());
        fx.write_slot("ab2c3d", &slot_text("", &good_config(false)));
        let main_before = fx.main_bytes();
        // when: the slot is prepared
        let run = fx.prepare("ab2c3d").expect("the slot is prepared");
        // then: the new secret is in the slot file with the rest kept, the main file is byte for byte the same
        let on_disk = fx.parsed("ab2c3d");
        assert_eq!(secret_bytes(&on_disk.secret), run.secret.to_bytes());
        assert_eq!(on_disk.created_unix, CREATED);
        assert_eq!(on_disk.net.network_key.as_deref(), Some(key().as_str()));
        assert!(run.file_report.persistent_identity);
        assert_eq!(fx.main_bytes(), main_before);
    }

    #[test]
    fn damaged_secret_keeps_a_copy_before_the_rewrite() {
        // given: a slot file whose secret is not base64
        let fx = setup(CliOverrides::default());
        fx.write_slot(
            "ab2c3d",
            &slot_text(r#""identity": {"secret": "broken"},"#, &good_config(false)),
        );
        let before = fx.slot_bytes("ab2c3d");
        // when: the slot is prepared
        let run = fx.prepare("ab2c3d").expect("the slot is prepared");
        // then: the old file is kept as a `.bad-` copy and the file holds the new secret
        let copy = fx
            .slot_dir("ab2c3d")
            .join(format!("{FILE_NAME}.bad-{}", stamp_utc(NOW)));
        assert_eq!(std::fs::read(copy).expect("the kept copy"), before);
        assert_eq!(
            secret_bytes(&fx.parsed("ab2c3d").secret),
            run.secret.to_bytes()
        );
    }

    #[test]
    fn failed_write_leaves_a_temporary_secret() {
        // given: a slot file without a secret, marked read-only
        let fx = setup(CliOverrides::default());
        fx.write_slot("ab2c3d", &slot_text("", &good_config(false)));
        let file = fx.slot_file("ab2c3d");
        let before = fx.slot_bytes("ab2c3d");
        let mut perms = std::fs::metadata(&file).expect("metadata").permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&file, perms).expect("marking read-only");
        // when: the slot is prepared
        let got = fx.prepare("ab2c3d");
        // then: it starts with a secret that is not persistent, and the report says why
        clear_readonly(&file);
        let run = got.expect("the slot is still prepared");
        assert!(!run.file_report.persistent_identity);
        assert!(run.file_report.message.is_some(), "the reason is missing");
        assert_eq!(fx.slot_bytes("ab2c3d"), before);
    }

    #[test]
    fn field_errors_alone_do_not_rewrite_the_file() {
        // given: a valid secret and a field with a wrong type
        let fx = setup(CliOverrides::default());
        let config = format!(
            r#", "network": {{"key": "{}"}}, "exit": {{"enabled": "maybe"}}"#,
            key()
        );
        fx.write_slot("ab2c3d", &slot_text(&secret_line(), &config));
        let before = fx.slot_bytes("ab2c3d");
        // when: the slot is prepared
        let run = fx.prepare("ab2c3d").expect("the slot is prepared");
        // then: the file and its folder are untouched, the error stays in the report
        assert_eq!(fx.slot_bytes("ab2c3d"), before);
        assert_eq!(names_in(&fx.slot_dir("ab2c3d")), ["config.json"]);
        assert!(!run.file_report.field_errors.is_empty());
    }
}

mod refused_slot {
    use super::*;

    #[test]
    fn no_network_key() {
        assert_refused_and_untouched(&slot_text(&secret_line(), ""));
    }

    #[test]
    fn network_key_that_does_not_parse() {
        let config = r#", "network": {"key": "mesh1-short"}"#;
        let err = assert_refused_and_untouched(&slot_text(&secret_line(), config));
        assert!(
            !err.contains("mesh1-short"),
            "the key value is in the message: {err}"
        );
    }

    #[test]
    fn network_key_of_the_wrong_type() {
        let config = r#", "network": {"key": 5}"#;
        assert_refused_and_untouched(&slot_text(&secret_line(), config));
    }

    #[test]
    fn file_that_is_not_json() {
        assert_refused_and_untouched("{ not json");
    }

    #[test]
    fn version_newer_than_the_program() {
        let text = slot_text(&secret_line(), &good_config(false))
            .replace(r#""version": 1"#, r#""version": 2"#);
        assert_refused_and_untouched(&text);
    }

    #[test]
    fn missing_file() {
        // given: a slot whose file vanished
        let fx = setup(CliOverrides::default());
        let path = fx.slot_file("ab2c3d");
        let slot = NetSlotOnDisk {
            id: slot_id("ab2c3d"),
            path,
            file: LoadedNetFile::Absent,
        };
        // when: the slot is prepared; then: refused and nothing is created
        assert!(prepare_net_slot(&fx.main, slot, NOW).is_err());
        assert!(!fx.slot_dir("ab2c3d").exists());
    }
}

struct Fixture {
    root: PathBuf,
    main_file: PathBuf,
    main: RunConfig,
}

impl Fixture {
    fn slot_dir(&self, id: &str) -> PathBuf {
        self.root.join("nets").join(id)
    }

    fn slot_file(&self, id: &str) -> PathBuf {
        self.slot_dir(id).join(FILE_NAME)
    }

    fn write_slot(&self, id: &str, text: &str) {
        std::fs::create_dir_all(self.slot_dir(id)).expect("slot folder");
        std::fs::write(self.slot_file(id), text).expect("slot file");
    }

    fn slot_bytes(&self, id: &str) -> Vec<u8> {
        std::fs::read(self.slot_file(id)).expect("reading the slot file")
    }

    fn main_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.main_file).expect("reading the main file")
    }

    fn parsed(&self, id: &str) -> crate::config::config_file::ParsedNetFile {
        match load_net_file(&self.slot_file(id)) {
            LoadedNetFile::Parsed(p) => p,
            other => panic!("expected Parsed, got {other:?}"),
        }
    }

    /// Reads the slot folder as the listing would and prepares it.
    fn prepare(&self, id: &str) -> Result<RunConfig, String> {
        let path = self.slot_file(id);
        let file = load_net_file(&path);
        let slot = NetSlotOnDisk {
            id: slot_id(id),
            path,
            file,
        };
        prepare_net_slot(&self.main, slot, NOW)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A main file with a secret, prepared with the given flags.
fn setup(cli: CliOverrides) -> Fixture {
    let root = scratch("slot-prepare");
    let main_file = root.join(FILE_NAME);
    save_config_file(&main_file, None, 1, &test_secret(), &Settings::default()).expect("main file");
    let source = ConfigSource::File {
        path: main_file.clone(),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let main = prepare(&source, &cli, false, NOW).expect("the main is prepared");
    Fixture {
        root,
        main_file,
        main,
    }
}

fn slot_id(text: &str) -> NetSlotId {
    NetSlotId::parse(text).expect("a valid slot id")
}

fn key() -> String {
    NetworkKey::from_bytes([5; 32]).encode()
}

fn secret_line() -> String {
    format!(r#""identity": {{"secret": "{SECRET_B64}"}},"#)
}

/// The `config` additions of a slot that can start: a key and the exit switch.
fn good_config(exit_enabled: bool) -> String {
    format!(
        r#", "network": {{"key": "{}"}}, "exit": {{"enabled": {exit_enabled}}}"#,
        key()
    )
}

/// A slot file: `identity` is spliced into the envelope (empty or ending with a comma), `config` goes
/// after `version` inside the `config` object.
fn slot_text(identity: &str, config: &str) -> String {
    format!(
        r#"{{"rev": 4, "saved_at": "2027-01-15T08:00:00Z", "created_unix": {CREATED},
        {identity} "config": {{"version": 1{config}}}}}"#
    )
}

fn secret_bytes(secret: &SecretRead) -> [u8; 32] {
    match secret {
        SecretRead::Valid(k) => k.to_bytes(),
        other => panic!("expected Valid, got {other:?}"),
    }
}

/// Prepares a slot with the given file text, checks it is refused with everything left as it was,
/// and returns the refusal text.
fn assert_refused_and_untouched(config_text: &str) -> String {
    // given: a slot file that must not start
    let fx = setup(CliOverrides::default());
    fx.write_slot("ab2c3d", config_text);
    let before = fx.slot_bytes("ab2c3d");
    let main_before = fx.main_bytes();
    // when: the slot is prepared
    let err = fx
        .prepare("ab2c3d")
        .err()
        .expect("the slot must be refused");
    // then: the file is not changed, not set aside and not copied; the main file is the same
    assert_eq!(fx.slot_bytes("ab2c3d"), before);
    assert_eq!(names_in(&fx.slot_dir("ab2c3d")), ["config.json"]);
    assert_eq!(fx.main_bytes(), main_before);
    err
}
