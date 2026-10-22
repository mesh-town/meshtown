//! Tests of the storage of last known member names (`peer_names.rs`).

use std::path::{Path, PathBuf};

use crate::transport::net::{NodeSecret, PeerId};

use super::*;
use crate::members::membership::Members;

#[path = "peer_names_load_tests.rs"]
mod load;

#[test]
fn remember_new_id_returns_true_and_goes_first() {
    let mut names = PeerNames::load(None, "net");
    let a = endpoint_id(1);
    let b = endpoint_id(2);
    names.remember(a, "A");

    let changed = names.remember(b, "B");

    assert!(changed, "a new id must return true");
    assert_eq!(names.ids(), vec![b, a], "a new record must go first");
}

#[test]
fn remember_same_name_returns_false_and_keeps_order() {
    let mut names = PeerNames::load(None, "net");
    let a = endpoint_id(1);
    let b = endpoint_id(2);
    names.remember(a, "A");
    names.remember(b, "B");
    assert_eq!(names.ids(), vec![b, a], "precondition: b is fresher than a");

    let changed = names.remember(a, "A");

    assert!(!changed, "the same name must not count as a change");
    assert_eq!(names.ids(), vec![b, a], "the order must not change");
}

#[test]
fn remember_new_name_replaces_and_moves_first() {
    let mut names = PeerNames::load(None, "net");
    let a = endpoint_id(1);
    let b = endpoint_id(2);
    names.remember(a, "A");
    names.remember(b, "B");

    let changed = names.remember(a, "New");

    assert!(changed, "another name must count as a change");
    assert_eq!(names.ids(), vec![a, b], "a changed record must go first");
    assert_eq!(names.get(a), Some("New"));
}

#[test]
fn remember_over_limit_drops_oldest() {
    let mut names = PeerNames::load(None, "net");
    for i in 0..MAX_NAMES {
        assert!(
            names.remember(wide_id(i as u32), &format!("Name-{i}")),
            "new member {i}"
        );
    }
    assert_eq!(
        names.len(),
        MAX_NAMES,
        "precondition: the storage is filled up to the ceiling"
    );

    let changed = names.remember(wide_id(MAX_NAMES as u32), "Newcomer");

    assert!(changed, "a new member must be accepted");
    assert_eq!(names.len(), MAX_NAMES, "the ceiling must not be exceeded");
    assert_eq!(
        names.get(wide_id(0)),
        None,
        "the oldest record must be evicted"
    );
    assert_eq!(names.get(wide_id(MAX_NAMES as u32)), Some("Newcomer"));
}

// ---------- writing (`save`) ----------

#[test]
fn save_then_load_roundtrip_keeps_order() {
    let dir = scratch("roundtrip");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let mut names = PeerNames::load(Some(&file), "net");
    let a = endpoint_id(1);
    let b = endpoint_id(2);
    let c = endpoint_id(3);
    names.remember(a, "A");
    names.remember(b, "B");
    names.remember(c, "C");
    names.save().expect("save must succeed");

    let loaded = PeerNames::load(Some(&file), "net");

    assert_eq!(
        loaded.ids(),
        vec![c, b, a],
        "the order after a round trip must be preserved"
    );
    assert_eq!(loaded.get(a), Some("A"));
    assert_eq!(loaded.get(b), Some("B"));
    assert_eq!(loaded.get(c), Some("C"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file)
            .expect("file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "permissions of the names file {mode:o}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_file_format_is_stable() {
    let dir = scratch("format");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let mut names = PeerNames::load(Some(&file), "net");
    let a = endpoint_id(1);
    names.remember(a, "Kitchen");
    names.save().expect("save must succeed");

    let text = std::fs::read_to_string(&file).expect("the file must be written");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("the file must be valid JSON");

    assert_eq!(value["version"].as_u64(), Some(1));
    assert_eq!(value["network_id"].as_str(), Some("net"));
    assert_eq!(
        value["peers"][0]["id"].as_str(),
        Some(a.to_string()).as_deref()
    );
    assert_eq!(value["peers"][0]["name"].as_str(), Some("Kitchen"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_without_path_is_ok_and_writes_nothing() {
    let mut names = PeerNames::load(None, "net");
    names.remember(endpoint_id(1), "Kitchen");

    assert!(
        names.save().is_ok(),
        "save without a path must not give an error"
    );
}

#[test]
fn save_without_directory_returns_error_with_path_not_panic() {
    let dir = scratch("no-dir");
    let file = dir.join("no-such-directory").join(PEER_NAMES_FILE_NAME);
    let mut names = PeerNames::load(Some(&file), "net");
    names.remember(endpoint_id(1), "Kitchen");

    let err = names
        .save()
        .expect_err("no directory — save must return Err");

    assert!(
        err.contains("no-such-directory") || err.contains(&file.display().to_string()),
        "the error text must contain the path: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- applying and a member's name ----------

#[test]
fn apply_to_sets_names_without_membership() {
    let mut names = PeerNames::load(None, "net");
    let a = endpoint_id(1);
    let b = endpoint_id(2);
    names.remember(a, "A");
    names.remember(b, "B");

    let members = Members::default();
    names.apply_to(&members);

    assert_eq!(members.name_of(a), Some("A".to_string()));
    assert_eq!(members.name_of(b), Some("B".to_string()));
    assert!(
        !members.is_member(a),
        "apply_to must not make the member live"
    );
    assert!(
        !members.is_member(b),
        "apply_to must not make the member live"
    );
    assert_eq!(
        members.count(),
        0,
        "apply_to must not call Members::add_verified_conn"
    );
}

#[test]
fn remember_member_saves_fresh_name() {
    let dir = scratch("remember-member");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let mut names = PeerNames::load(Some(&file), "net");
    let members = Members::default();
    let a = endpoint_id(1);
    members.set_name(a, "Kitchen".to_string());

    names.remember_member_and_save(&members, a);

    let loaded = PeerNames::load(Some(&file), "net");
    assert_eq!(
        loaded.get(a),
        Some("Kitchen"),
        "the member's name must reach the disk"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remember_member_without_name_writes_nothing() {
    let dir = scratch("no-name");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let mut names = PeerNames::load(Some(&file), "net");
    let members = Members::default();
    let a = endpoint_id(1);

    names.remember_member_and_save(&members, a);

    assert!(
        !file.exists(),
        "without a name in Members the file must not appear"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn remember_member_same_name_skips_write() {
    let dir = scratch("same-name");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let mut names = PeerNames::load(Some(&file), "net");
    let members = Members::default();
    let a = endpoint_id(1);
    members.set_name(a, "Kitchen".to_string());
    names.remember_member_and_save(&members, a);
    assert!(
        file.exists(),
        "precondition: the first remembering must write the file"
    );
    std::fs::remove_file(&file).expect("delete the file before the repeat");

    names.remember_member_and_save(&members, a);

    assert!(
        !file.exists(),
        "the same name must not cause a repeated write"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- helpers ----------

/// A temporary directory of the test; at the end of the test the caller itself calls `remove_dir_all`.
fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-peer-names-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("could not create the test's temporary directory");
    dir
}

/// `PeerId` of member `n`, for calls to `remember`/`get`. Up to 256 distinct values.
fn endpoint_id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

/// `PeerId` by a wide index — where more than 256 distinct members are needed
/// (`endpoint_id` gives at most 256 values, and `MAX_NAMES` is already 256).
fn wide_id(n: u32) -> PeerId {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&n.to_be_bytes());
    NodeSecret::from_bytes(&bytes).peer_id()
}

/// Writes a test names file in the `NamesFile` format directly — for the cases of corruption, version, network,
/// bad ids and bad names.
fn write_names_file(path: &Path, version: u8, network_id: &str, peers: Vec<NameEntry>) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the test directory");
    }
    let content = serde_json::to_string(&NamesFile {
        version,
        network_id: network_id.to_string(),
        peers,
    })
    .expect("serialize the test names file");
    std::fs::write(path, content).expect("write the test names file");
}
