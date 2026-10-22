//! Tests for `file.rs`: the format, reading with `.bak`, atomic writing, quarantine.

use super::*;
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-replica-file-{}-{}", std::process::id(), seq));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn rid(n: u8) -> ReplicaId {
    ReplicaId([n; 32])
}

fn entry(n: u8) -> WireEntry {
    WireEntry {
        author: [n; 32],
        key: vec![n, 1],
        value: vec![n, 2, 3],
        ts_micros: u64::from(n) * 10,
        sig: [[n; 32], [n.wrapping_add(1); 32]],
    }
}

fn mark() -> ImportMark {
    ImportMark { at_micros: 77, source: MarkSource::DocsStore, entries: 3 }
}

fn bytes_of(n: u8) -> Vec<u8> {
    encode(&rid(1), None, &[entry(n)])
}

fn bak_of(path: &Path) -> PathBuf {
    bak_path(path)
}

#[test]
fn roundtrip_encode_decode_with_mark() {
    let es = vec![entry(1), entry(2)];
    let bytes = encode(&rid(1), Some(&mark()), &es);
    assert_eq!(&bytes[..4], b"MVRP");
    let (m, got) = decode(&rid(1), &bytes).unwrap();
    assert_eq!(m, Some(mark()));
    assert_eq!(got, es);
}

#[test]
fn decode_checksum_flip_is_broken() {
    let mut bytes = bytes_of(1);
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    assert!(matches!(decode(&rid(1), &bytes), Err(FileError::BadChecksum)));
}

#[test]
fn decode_other_replica_is_error() {
    let bytes = bytes_of(1);
    assert!(matches!(decode(&rid(2), &bytes), Err(FileError::OtherReplica)));
}

#[test]
fn decode_bad_magic_or_version_is_error() {
    let mut bad_magic = bytes_of(1);
    bad_magic[0] = b'X';
    assert!(matches!(decode(&rid(1), &bad_magic), Err(FileError::BadMagic)));
    let mut bad_ver = bytes_of(1);
    bad_ver[5] = 2;
    assert!(matches!(decode(&rid(1), &bad_ver), Err(FileError::BadVersion(2))));
    assert!(matches!(decode(&rid(1), b"MV"), Err(FileError::BadMagic)));
}

#[test]
fn path_for_names() {
    let d = Path::new("/x/replica");
    assert_eq!(path_for(d, "domains", &rid(0xab)), d.join("domains.bin"));
    assert_eq!(path_for(d, "net", &rid(0xab)), d.join("net-ababababab.bin"));
    assert_eq!(replica_dir(Path::new("/c/docs-store")), Some(PathBuf::from("/c/replica")));
}

#[test]
fn load_fresh_when_no_files() {
    let dir = temp_dir();
    let p = dir.join("net.bin");
    assert!(matches!(load(&p, &rid(1)), Ok(Loaded::Fresh)));
}

#[test]
fn load_main_when_whole() {
    let dir = temp_dir();
    let p = dir.join("net.bin");
    save(&p, &bytes_of(1), Bak::Keep).unwrap();
    assert!(matches!(load(&p, &rid(1)), Ok(Loaded::Main(None, e)) if e == vec![entry(1)]));
}

#[test]
fn load_backup_when_main_broken() {
    let dir = temp_dir();
    let p = dir.join("net.bin");
    save(&p, &bytes_of(1), Bak::Mirror).unwrap();
    std::fs::write(&p, b"garbage").unwrap();
    assert!(matches!(load(&p, &rid(1)), Ok(Loaded::Backup(None, e)) if e == vec![entry(1)]));
}

#[test]
fn load_backup_when_main_missing() {
    let dir = temp_dir();
    let p = dir.join("net.bin");
    save(&p, &bytes_of(1), Bak::Mirror).unwrap();
    std::fs::remove_file(&p).unwrap();
    assert!(matches!(load(&p, &rid(1)), Ok(Loaded::Backup(..))));
}

#[test]
fn load_both_broken_is_error_with_paths() {
    let dir = temp_dir();
    let p = dir.join("net.bin");
    std::fs::write(&p, b"garbage").unwrap();
    std::fs::write(bak_of(&p), b"garbage too").unwrap();
    match load(&p, &rid(1)) {
        Err(FileError::Broken { main, bak, .. }) => {
            assert_eq!(main, p);
            assert_eq!(bak, bak_of(&p));
        }
        _ => panic!("expected Broken"),
    }
    assert!(format!("{}", load(&p, &rid(1)).err().unwrap()).contains("net.bin"));
}

#[test]
fn save_copy_previous_keeps_previous() {
    let dir = temp_dir();
    let p = dir.join("net.bin");
    save(&p, &bytes_of(1), Bak::Keep).unwrap();
    save(&p, &bytes_of(2), Bak::CopyPrevious).unwrap();
    assert_eq!(std::fs::read(bak_of(&p)).unwrap(), bytes_of(1));
    assert_eq!(std::fs::read(&p).unwrap(), bytes_of(2));
}

#[test]
fn save_mirror_writes_same_bytes() {
    let dir = temp_dir();
    let p = dir.join("domains.bin");
    save(&p, &bytes_of(2), Bak::Mirror).unwrap();
    assert_eq!(std::fs::read(bak_of(&p)).unwrap(), bytes_of(2));
    assert_eq!(std::fs::read(&p).unwrap(), bytes_of(2));
}

#[test]
fn save_leaves_no_tmp() {
    let dir = temp_dir();
    let p = dir.join("domains.bin");
    save(&p, &bytes_of(1), Bak::Mirror).unwrap();
    save(&p, &bytes_of(2), Bak::Mirror).unwrap();
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.contains(".tmp")), "{names:?}");
}

#[cfg(unix)]
#[test]
fn save_private_modes() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir();
    let sub = dir.join("replica");
    let p = sub.join("domains.bin");
    save(&p, &bytes_of(1), Bak::Mirror).unwrap();
    let mode = |q: &Path| std::fs::metadata(q).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&sub), 0o700);
    assert_eq!(mode(&p), 0o600);
    assert_eq!(mode(&bak_of(&p)), 0o600);
}

#[test]
fn quarantine_renames_with_broken_suffix() {
    let dir = temp_dir();
    let p = dir.join("net.bin");
    std::fs::write(&p, b"garbage").unwrap();
    let q = quarantine(&p).unwrap();
    assert!(!p.exists());
    assert_eq!(std::fs::read(&q).unwrap(), b"garbage");
    let name = q.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("net.bin.broken-"), "{name}");
    assert!(name["net.bin.broken-".len()..].parse::<u64>().is_ok());
}
