use super::*;

const H1: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const H2: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";
const NOW: u64 = 1_000_000;

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mesh-kept-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn dealt(text: &str) -> Dealt {
    Dealt {
        text: text.to_string(),
        qr_text: format!("{text}-short"),
        holders: Vec::new(),
    }
}

fn open(hashes: &[&str]) -> BTreeSet<String> {
    hashes.iter().map(|h| h.to_string()).collect()
}

#[test]
fn keeps_and_reads_back_by_hash() {
    let d = dir("round");
    let path = d.join(KEPT_INVITES_FILE_NAME);
    keep(&path, H1, NOW, &dealt("one")).unwrap();
    keep(&path, H2, NOW + 1, &dealt("two")).unwrap();
    let one = kept(&path, H1).unwrap();
    assert_eq!((one.made_unix, one.text.as_str()), (NOW, "one"));
    assert_eq!(one.qr_text, "one-short");
    assert_eq!(kept(&path, H2).unwrap().text, "two");
    assert!(kept(&path, &"ab".repeat(32)).is_none());
    std::fs::remove_dir_all(&d).ok();
}

#[cfg(unix)]
#[test]
fn file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let d = dir("mode");
    let path = d.join(KEPT_INVITES_FILE_NAME);
    keep(&path, H1, NOW, &dealt("one")).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn prune_drops_not_open_and_expired() {
    let d = dir("prune");
    let path = d.join(KEPT_INVITES_FILE_NAME);
    let old = NOW - INVITE_TTL_SECS - 1;
    keep(&path, H1, NOW, &dealt("live")).unwrap();
    keep(&path, H2, old, &dealt("old")).unwrap();
    keep(&path, &"ab".repeat(32), NOW, &dealt("used")).unwrap();
    prune(&path, &open(&[H1, H2]), NOW).unwrap();
    assert_eq!(kept_hashes(&path), open(&[H1]));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn prune_to_nothing_removes_the_file() {
    let d = dir("gone");
    let path = d.join(KEPT_INVITES_FILE_NAME);
    keep(&path, H1, NOW, &dealt("one")).unwrap();
    prune(&path, &open(&[]), NOW).unwrap();
    assert!(!path.exists());
    prune(&path, &open(&[]), NOW).unwrap();
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn broken_file_reads_as_empty() {
    let d = dir("broken");
    let path = d.join(KEPT_INVITES_FILE_NAME);
    assert!(kept(&path, H1).is_none());
    std::fs::write(&path, "{ not json").unwrap();
    assert!(kept(&path, H1).is_none());
    prune(&path, &open(&[H1]), NOW).unwrap();
    keep(&path, H1, NOW, &dealt("fresh")).unwrap();
    assert_eq!(kept(&path, H1).unwrap().text, "fresh");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn debug_does_not_show_the_text() {
    let record = KeptInvite {
        made_unix: NOW,
        text: "SECRET-TEXT".to_string(),
        qr_text: "SECRET-QR".to_string(),
    };
    let shown = format!("{record:?}");
    assert!(!shown.contains("SECRET"), "{shown}");
}
