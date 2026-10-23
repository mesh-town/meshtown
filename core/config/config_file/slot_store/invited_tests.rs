use super::*;
use crate::transport::net::NodeSecret;

fn peer(n: u8) -> PeerAddr {
    PeerAddr::new(NodeSecret::from_bytes(&[n; 32]).peer_id())
}

fn slot_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mesh-invited-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn reads_back_what_was_written_in_order() {
    let dir = slot_dir("round");
    let file = dir.join("config.json");
    let peers = vec![peer(3), peer(1), peer(2)];
    save_invited_peers(&file, &peers).unwrap();
    assert_eq!(load_invited_peers(&file), peers);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn writes_nothing_for_an_empty_list() {
    let dir = slot_dir("empty");
    let file = dir.join("config.json");
    save_invited_peers(&file, &[]).unwrap();
    assert!(!dir.join(INVITED_PEERS_FILE_NAME).exists());
    assert!(load_invited_peers(&file).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn skips_lines_that_are_not_peer_ids() {
    let dir = slot_dir("junk");
    let file = dir.join("config.json");
    let good = peer(5);
    let text = format!("garbage\n\n{}\n12345\n", good.id);
    std::fs::write(dir.join(INVITED_PEERS_FILE_NAME), text).unwrap();
    assert_eq!(load_invited_peers(&file), vec![good]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn collapses_repeated_peers() {
    let dir = slot_dir("dups");
    let file = dir.join("config.json");
    save_invited_peers(&file, &[peer(1), peer(2), peer(1)]).unwrap();
    assert_eq!(load_invited_peers(&file), vec![peer(1), peer(2)]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_missing_file_is_an_empty_list() {
    let dir = slot_dir("missing");
    assert!(load_invited_peers(&dir.join("config.json")).is_empty());
    std::fs::remove_dir_all(&dir).ok();
}
