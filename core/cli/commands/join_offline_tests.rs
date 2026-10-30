use super::*;
use crate::config::config_file::PathRule;
use crate::invite::{InviteHead, InviteSecret};
use crate::members::network_key::managed_key_with_label;
use crate::transport::net::NodeSecret;

/// A directory for the settings file; removed on drop.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let dir = std::env::temp_dir().join(format!("join_offline-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }

    fn file(&self) -> PathBuf {
        self.0.join("config.json")
    }

    fn source(&self) -> ConfigSource {
        ConfigSource::File {
            path: self.file(),
            may_create: true,
            rule: PathRule::Explicit,
        }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key_of(seed: u8) -> String {
    let creator = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    managed_key_with_label(None, &[seed; 32], &creator)
}

fn invite_made_at(made_unix: u64) -> ParsedInvite {
    let creator = NodeSecret::from_bytes(&[1u8; 32]).peer_id();
    let network_id = parse_user_input(&key_of(9))
        .unwrap()
        .key
        .subkeys()
        .network_id;
    let head = InviteHead {
        secret: InviteSecret::from_bytes([0xC7; 32]),
        network_id,
        label: None,
        made_unix,
        root: creator,
    };
    ParsedInvite {
        head,
        peers: vec![],
        warnings: vec![],
    }
}

#[tokio::test]
async fn key_into_absent_file_creates_it_with_key() {
    // given: no settings file
    let dir = Dir::new("absent");
    let key = key_of(9);
    // when
    let joined = join_offline(&dir.source(), JoinText::Key(key.clone()), 100)
        .await
        .unwrap();
    // then: the file exists and holds the key
    assert!(!joined.already);
    assert_eq!(joined.path, dir.file());
    assert_eq!(peek_file_key(&dir.file()).as_deref(), Some(key.as_str()));
}

#[tokio::test]
async fn key_of_same_network_is_already() {
    // given: the key is already in the file
    let dir = Dir::new("same");
    let key = key_of(9);
    join_offline(&dir.source(), JoinText::Key(key.clone()), 100)
        .await
        .unwrap();
    let before = std::fs::read(dir.file()).unwrap();
    // when: the same key again
    let joined = join_offline(&dir.source(), JoinText::Key(key), 200)
        .await
        .unwrap();
    // then: nothing changes
    assert!(joined.already);
    assert_eq!(std::fs::read(dir.file()).unwrap(), before);
}

#[tokio::test]
async fn offline_key_for_other_network_adds_slot() {
    // given: the file holds network A
    let dir = Dir::new("other");
    join_offline(&dir.source(), JoinText::Key(key_of(9)), 100)
        .await
        .unwrap();
    // when: the key of network B
    let other = key_of(8);
    let joined = join_offline(&dir.source(), JoinText::Key(other.clone()), 200)
        .await
        .unwrap();
    // then: B is a slot, A stays the main network
    assert!(!joined.already);
    assert_eq!(saved_slot_keys(&dir.file()), vec![Some(other)]);
    assert_eq!(
        peek_file_key(&dir.file()).as_deref(),
        Some(key_of(9).as_str())
    );
}

#[tokio::test]
async fn expired_invite_is_refused_before_any_write() {
    // given: an invite made long ago, no settings file
    let dir = Dir::new("expired");
    let text = JoinText::Invite(invite_made_at(1));
    // when
    let err = join_offline(&dir.source(), text, 10_000_000)
        .await
        .err()
        .expect("refused");
    // then: no file was created
    assert!(format!("{err:#}").contains("has run out"), "{err:#}");
    assert!(!dir.file().exists());
}

#[tokio::test]
async fn unreadable_file_is_refused_untouched() {
    // given: a settings file that is not a settings file
    let dir = Dir::new("unreadable");
    std::fs::write(dir.file(), b"{ not json").unwrap();
    // when
    let err = join_offline(&dir.source(), JoinText::Key(key_of(9)), 100)
        .await
        .err()
        .expect("refused");
    // then: the bytes are the same
    assert!(format!("{err:#}").contains("cannot be read"), "{err:#}");
    assert_eq!(std::fs::read(dir.file()).unwrap(), b"{ not json");
}

#[test]
fn busy_page_port_stops_before_exchange() {
    // given: something listens on the address the page would take (the default is every interface)
    let held = TcpListener::bind("0.0.0.0:0").unwrap();
    let port = held.local_addr().unwrap().port();
    let dir = Dir::new("busy");
    // when
    let err = hold_page_port(&dir.file(), port).expect_err("busy");
    // then
    assert!(
        err.to_string().contains(&format!("port {port} is in use")),
        "{err}"
    );
}

#[test]
fn free_page_port_is_held_until_dropped() {
    // given: a free port
    let dir = Dir::new("free");
    let port = {
        let probe = TcpListener::bind("0.0.0.0:0").unwrap();
        probe.local_addr().unwrap().port()
    };
    // when / then: the first hold works, the second fails while the first lives
    let first = hold_page_port(&dir.file(), port).unwrap();
    assert!(hold_page_port(&dir.file(), port).is_err());
    drop(first);
}

#[tokio::test]
async fn key_saved_is_false_without_the_key_in_file() {
    // given: no file, then a file with the key
    let dir = Dir::new("saved");
    let key = key_of(9);
    assert!(!key_saved(&dir.file(), &key));
    // when
    join_offline(&dir.source(), JoinText::Key(key.clone()), 100)
        .await
        .unwrap();
    // then
    assert!(key_saved(&dir.file(), &key));
    assert!(!key_saved(&dir.file(), &key_of(8)));
}
