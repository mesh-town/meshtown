//! Golden test of the `peer-names.json` format (member id is a string of 64 lowercase hex): the expectations are
//! literals that survive a replacement of the id type. A separate neighbor file: `peer_names_tests.rs` is at the ceiling.

use crate::transport::net::{NodeSecret, PeerId};

use super::*;

fn id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

#[test]
fn golden_names_file_stores_id_as_lowercase_hex_string() {
    const KEY1: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    let id = id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    // RFC 8032 §7.1 anchor
    assert_eq!(id.to_string(), KEY1);
    let dir = std::env::temp_dir().join(format!("mesh-peer-names-golden-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("test directory");
    let file = dir.join(PEER_NAMES_FILE_NAME);

    let mut names = PeerNames::load(Some(&file), "net");
    assert!(names.remember(id, "ginger"));
    names.save().expect("save");

    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("file")).expect("JSON");
    let want = serde_json::json!({
        "version": 1,
        "network_id": "net",
        "peers": [{
            "id": "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "name": "ginger"
        }]
    });
    assert_eq!(doc, want);

    // the same id in capitals is not read from the file — the record is skipped
    let upper = r#"{"version":1,"network_id":"net","peers":[{"id":"D75A980182B10AB7D54BFED3C964073A0EE172F3DAA62325AF021A68F707511A","name":"ginger"}]}"#;
    std::fs::write(&file, upper).expect("write the literal");
    assert_eq!(PeerNames::load(Some(&file), "net").len(), 0);

    // as a lowercase record — it is read (a check that the emptiness above is not from other corruption)
    let lower = upper.replace(
        "D75A980182B10AB7D54BFED3C964073A0EE172F3DAA62325AF021A68F707511A",
        KEY1,
    );
    std::fs::write(&file, lower).expect("write the literal");
    assert_eq!(PeerNames::load(Some(&file), "net").len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
