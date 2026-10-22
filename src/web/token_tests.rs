//! Tests for `token.rs`: generation, file permissions, reuse and replacement of the token.

use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> std::path::PathBuf {
    // `SystemTime::now()` paired with `process::id()` does not guarantee uniqueness:
    // on macOS the clock resolution is coarser than a nanosecond, and `cargo test` runs tests
    // in parallel threads of one process — two threads can get
    // the same `nanos` and collide on one directory. An atomic counter
    // makes every call unique regardless of the clock resolution.
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "mesh-web-token-{}-{}-{}",
        std::process::id(),
        nanos,
        seq
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(unix)]
fn file_mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[cfg(unix)]
fn has_tmp_leftover(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".web-token.tmp-")
        })
}

#[test]
fn generate_is_well_formed_and_random() {
    let a = generate().unwrap();
    let b = generate().unwrap();
    assert_eq!(a.len(), TOKEN_LEN);
    assert!(is_well_formed(&a));
    assert_ne!(a, b);
}

#[test]
#[cfg(unix)]
fn obtain_creates_file_with_right_perms() {
    let dir = temp_dir();
    let token = resolve_web_token(Some(&dir), None).unwrap();
    let path = dir.join(super::super::TOKEN_FILE_NAME);
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.trim_end_matches('\n'), token);
    assert_eq!(file_mode(&path), 0o600);
    assert!(!has_tmp_leftover(&dir));
    cleanup(&dir);
}

#[test]
fn obtain_reuses_existing_token() {
    let dir = temp_dir();
    let first = resolve_web_token(Some(&dir), None).unwrap();
    let second = resolve_web_token(Some(&dir), None).unwrap();
    assert_eq!(first, second);
    cleanup(&dir);
}

#[test]
#[cfg(unix)]
fn obtain_replaces_token_on_bad_perms() {
    let dir = temp_dir();
    let first = resolve_web_token(Some(&dir), None).unwrap();
    let path = dir.join(super::super::TOKEN_FILE_NAME);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let third = resolve_web_token(Some(&dir), None).unwrap();
    assert_ne!(first, third);
    assert_eq!(file_mode(&path), 0o600);
    cleanup(&dir);
}

#[test]
fn obtain_replaces_garbage_content() {
    let dir = temp_dir();
    let path = dir.join(super::super::TOKEN_FILE_NAME);
    std::fs::write(&path, "abc").unwrap();
    let token = resolve_web_token(Some(&dir), None).unwrap();
    assert!(is_well_formed(&token));
    assert_ne!(token, "abc");
    cleanup(&dir);
}

#[test]
fn obtain_without_dir_keeps_memory_only() {
    let token = resolve_web_token(None, None).unwrap();
    assert!(is_well_formed(&token));
}

#[test]
fn obtain_with_fixed_token_writes_it() {
    let dir = temp_dir();
    let token = resolve_web_token(Some(&dir), Some("fixed-token".to_string())).unwrap();
    assert_eq!(token, "fixed-token");
    let path = dir.join(super::super::TOKEN_FILE_NAME);
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.trim_end_matches('\n'), "fixed-token");
    cleanup(&dir);
}
