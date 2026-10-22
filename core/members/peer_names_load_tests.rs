//! Tests of `peer_names.rs`: the file path and reading (`load`).

use super::*;

#[test]
fn path_beside_replaces_config_file_name() {
    let config = Path::new("/a/b/config.json");
    let names_path = peer_names_path_beside(config).expect("the path has a parent");
    assert_eq!(names_path, PathBuf::from("/a/b/peer-names.json"));
}

// ---------- reading → no data ----------

#[test]
fn load_without_path_is_empty() {
    let names = PeerNames::load(None, "net");
    assert!(names.is_empty(), "without a path the storage must be empty");
    assert_eq!(names.len(), 0);
}

#[test]
fn load_missing_file_is_empty() {
    let dir = scratch("missing-file");
    let file = dir.join(PEER_NAMES_FILE_NAME);

    let names = PeerNames::load(Some(&file), "net");

    assert!(names.is_empty());
    assert!(!file.exists(), "load must not create a file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_missing_directory_is_empty() {
    let dir = scratch("missing-dir");
    let file = dir.join("no-such-directory").join(PEER_NAMES_FILE_NAME);

    let names = PeerNames::load(Some(&file), "net");

    assert!(
        names.is_empty(),
        "a nonexistent directory must read as an empty storage, without a panic"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- reading → a bad file as a whole ----------

#[test]
fn load_bad_json_is_empty() {
    let dir = scratch("bad-json");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    std::fs::write(&file, "not json").expect("write a broken file");

    let names = PeerNames::load(Some(&file), "net");

    assert!(
        names.is_empty(),
        "corrupt JSON must read as an empty storage"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_other_version_is_empty() {
    let dir = scratch("other-version");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    write_names_file(
        &file,
        PEER_NAMES_FORMAT_VERSION + 1,
        "net",
        vec![NameEntry {
            id: endpoint_id(1).to_string(),
            name: "Kitchen".to_string(),
        }],
    );

    let names = PeerNames::load(Some(&file), "net");

    assert!(
        names.is_empty(),
        "a foreign file version must read as an empty storage"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_other_network_is_empty() {
    let dir = scratch("other-network");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    write_names_file(
        &file,
        PEER_NAMES_FORMAT_VERSION,
        "another-network",
        vec![NameEntry {
            id: endpoint_id(1).to_string(),
            name: "Kitchen".to_string(),
        }],
    );

    let names = PeerNames::load(Some(&file), "net");

    assert!(
        names.is_empty(),
        "a foreign network's file must read as an empty storage"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- reading → bad records ----------

#[test]
fn load_skips_bad_id() {
    let dir = scratch("bad-id");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    write_names_file(
        &file,
        PEER_NAMES_FORMAT_VERSION,
        "net",
        vec![
            NameEntry {
                id: "xyz".to_string(),
                name: "Kitchen".to_string(),
            },
            NameEntry {
                id: endpoint_id(1).to_string(),
                name: "Bedroom".to_string(),
            },
        ],
    );

    let names = PeerNames::load(Some(&file), "net");

    assert_eq!(
        names.len(),
        1,
        "a record with an unparseable id must be skipped"
    );
    assert_eq!(names.get(endpoint_id(1)), Some("Bedroom"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_skips_invalid_name() {
    let dir = scratch("bad-name");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let too_long = "a".repeat(65);
    write_names_file(
        &file,
        PEER_NAMES_FORMAT_VERSION,
        "net",
        vec![
            NameEntry {
                id: endpoint_id(1).to_string(),
                name: "".to_string(),
            },
            NameEntry {
                id: endpoint_id(2).to_string(),
                name: "   ".to_string(),
            },
            NameEntry {
                id: endpoint_id(3).to_string(),
                name: too_long,
            },
            NameEntry {
                id: endpoint_id(4).to_string(),
                name: "a\u{2028}b".to_string(),
            },
            NameEntry {
                id: endpoint_id(5).to_string(),
                name: "Kitchen".to_string(),
            },
        ],
    );

    let names = PeerNames::load(Some(&file), "net");

    assert_eq!(names.len(), 1, "only the valid record must remain");
    assert_eq!(names.get(endpoint_id(5)), Some("Kitchen"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_trims_name() {
    let dir = scratch("trim-name");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    write_names_file(
        &file,
        PEER_NAMES_FORMAT_VERSION,
        "net",
        vec![NameEntry {
            id: endpoint_id(1).to_string(),
            name: "  Kitchen  ".to_string(),
        }],
    );

    let names = PeerNames::load(Some(&file), "net");

    assert_eq!(
        names.get(endpoint_id(1)),
        Some("Kitchen"),
        "the name must be trimmed at the edges"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_keeps_first_of_duplicate_ids() {
    let dir = scratch("dup-id");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let id = endpoint_id(1).to_string();
    write_names_file(
        &file,
        PEER_NAMES_FORMAT_VERSION,
        "net",
        vec![
            NameEntry {
                id: id.clone(),
                name: "A".to_string(),
            },
            NameEntry {
                id,
                name: "B".to_string(),
            },
        ],
    );

    let names = PeerNames::load(Some(&file), "net");

    assert_eq!(
        names.len(),
        1,
        "a repeated id must collapse into one record"
    );
    assert_eq!(
        names.get(endpoint_id(1)),
        Some("A"),
        "the first (freshest) record must remain"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_truncates_to_max_names() {
    let dir = scratch("truncate");
    let file = dir.join(PEER_NAMES_FILE_NAME);
    let total = MAX_NAMES + 5;
    let peers: Vec<NameEntry> = (0..total)
        .map(|i| NameEntry {
            id: wide_id(i as u32).to_string(),
            name: format!("Name-{i}"),
        })
        .collect();
    write_names_file(&file, PEER_NAMES_FORMAT_VERSION, "net", peers);

    let names = PeerNames::load(Some(&file), "net");

    assert_eq!(
        names.len(),
        MAX_NAMES,
        "beyond the ceiling exactly MAX_NAMES records must remain"
    );
    assert_eq!(
        names.get(wide_id(0)),
        Some("Name-0"),
        "the very first (freshest) record must be kept"
    );
    assert_eq!(
        names.get(wide_id((total - 1) as u32)),
        None,
        "a record beyond the ceiling must be cut off"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- remembering (`remember`) ----------
