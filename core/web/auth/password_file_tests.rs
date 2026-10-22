//! Tests for `password_file.rs`: reading, writing, deleting and parsing bad files.

use super::*;
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mesh-web-password-{}-{}-{}",
        std::process::id(),
        nanos,
        seq
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn record(set_at: u64) -> PasswordRecord {
    PasswordRecord {
        iterations: NonZeroU32::new(1000).unwrap(),
        salt: [7u8; 16],
        hash: [9u8; 32],
        set_at,
    }
}

fn file_in(dir: &std::path::Path) -> PathBuf {
    dir.join(PASSWORD_FILE_NAME)
}

mod no_file {
    use super::*;

    #[test]
    fn read_is_none() {
        let dir = temp_dir();
        assert_eq!(read_password_file(&dir), PasswordFileState::Absent);
    }

    #[test]
    fn remove_is_ok() {
        let dir = temp_dir();
        assert!(remove_password_file(&dir).is_ok());
    }
}

mod written {
    use super::*;

    #[test]
    fn read_returns_same_record() {
        let dir = temp_dir();
        write_password_file(&dir, &record(5)).unwrap();
        assert_eq!(read_password_file(&dir), PasswordFileState::Set(record(5)));
    }

    #[cfg(unix)]
    #[test]
    fn mode_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        write_password_file(&dir, &record(5)).unwrap();
        let mode = std::fs::metadata(file_in(&dir)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn no_tmp_left() {
        let dir = temp_dir();
        write_password_file(&dir, &record(5)).unwrap();
        let left = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains(".tmp-"));
        assert!(!left);
    }

    #[test]
    fn text_is_one_line_with_format_keys() {
        let dir = temp_dir();
        write_password_file(&dir, &record(5)).unwrap();
        let text = std::fs::read_to_string(file_in(&dir)).unwrap();
        assert_eq!(text.matches('\n').count(), 1);
        assert!(text.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["alg"], "pbkdf2-sha256");
        assert_eq!(v["iterations"], 1000);
        assert_eq!(v["set_at"], 5);
        assert!(v["salt"].is_string() && v["hash"].is_string());
    }

    #[test]
    fn second_write_replaces_first() {
        let dir = temp_dir();
        write_password_file(&dir, &record(1)).unwrap();
        write_password_file(&dir, &record(2)).unwrap();
        assert_eq!(read_password_file(&dir), PasswordFileState::Set(record(2)));
    }

    #[test]
    fn remove_deletes_file() {
        let dir = temp_dir();
        write_password_file(&dir, &record(1)).unwrap();
        remove_password_file(&dir).unwrap();
        assert_eq!(read_password_file(&dir), PasswordFileState::Absent);
    }
}

mod bad_file {
    use super::*;

    fn good_json() -> serde_json::Value {
        let b64 = |n: usize| data_encoding::BASE64.encode(&vec![1u8; n]);
        serde_json::json!({
            "version": 1, "alg": "pbkdf2-sha256", "iterations": 1000,
            "salt": b64(16), "hash": b64(32), "set_at": 1
        })
    }

    fn put(dir: &std::path::Path, text: &str) {
        write_tmp_and_rename(&dir.join(".t"), &file_in(dir), text).unwrap();
    }

    fn unreadable(v: serde_json::Value) -> bool {
        let dir = temp_dir();
        put(&dir, &v.to_string());
        matches!(read_password_file(&dir), PasswordFileState::Unreadable(_))
    }

    #[test]
    fn garbage_is_unreadable() {
        let dir = temp_dir();
        put(&dir, "not json");
        assert!(matches!(read_password_file(&dir), PasswordFileState::Unreadable(_)));
    }

    #[test]
    fn version_2_is_unreadable() {
        let mut v = good_json();
        v["version"] = 2.into();
        assert!(unreadable(v));
    }

    #[test]
    fn unknown_alg_is_unreadable() {
        let mut v = good_json();
        v["alg"] = "argon2".into();
        assert!(unreadable(v));
    }

    #[test]
    fn zero_iterations_is_unreadable() {
        let mut v = good_json();
        v["iterations"] = 0.into();
        assert!(unreadable(v));
    }

    #[test]
    fn short_salt_is_unreadable() {
        let mut v = good_json();
        v["salt"] = data_encoding::BASE64.encode(&[1u8; 8]).into();
        assert!(unreadable(v));
    }

    #[test]
    fn over_4_kib_is_unreadable() {
        let mut v = good_json();
        v["pad"] = "x".repeat(5000).into();
        assert!(unreadable(v));
    }

    #[cfg(unix)]
    #[test]
    fn mode_0644_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        write_password_file(&dir, &record(1)).unwrap();
        let perms = std::fs::Permissions::from_mode(0o644);
        std::fs::set_permissions(file_in(&dir), perms).unwrap();
        assert!(matches!(read_password_file(&dir), PasswordFileState::Unreadable(_)));
    }

    #[test]
    fn extra_key_is_tolerated() {
        let mut v = good_json();
        v["note"] = "extra".into();
        let dir = temp_dir();
        put(&dir, &v.to_string());
        assert!(matches!(read_password_file(&dir), PasswordFileState::Set(_)));
    }
}
