//! `POST /api/network/join` with an invite string: an invite that cannot be used is refused before any
//! network is touched.

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::web_gui::routes::invite_fixture::{invite_into, stale_invite_into, word_key};
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};
use crate::web_gui::SlotHandle;

fn empty_slot() -> SlotHandle {
    slot_handle(
        "main",
        SnapNode::new(Default::default()),
        config_with_key(None),
    )
}

async fn when_join(slot: &SlotHandle, text: &str) -> (StatusCode, Value) {
    let router = test_app(FakeHost::new(vec![slot.clone()])).0;
    let body = json!({"key": text}).to_string();
    send(&router, Method::POST, "/api/network/join", &body).await
}

fn key_of(slot: &SlotHandle) -> Option<String> {
    slot.config.lock().unwrap().current_network_key.clone()
}

#[tokio::test]
async fn an_expired_invite_is_refused_and_writes_no_key() {
    let slot = empty_slot();
    let invite = stale_invite_into(&word_key("beta"));
    let (status, reply) = when_join(&slot, &invite).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_eq!(key_of(&slot), None);
}

#[tokio::test]
async fn a_cut_invite_is_refused_and_writes_no_key() {
    let slot = empty_slot();
    let invite = invite_into(&word_key("beta"));
    let (status, reply) = when_join(&slot, &invite[..invite.len() - 6]).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{reply}");
    assert_eq!(reply["error"], "invalid_key", "{reply}");
    assert_eq!(key_of(&slot), None);
}

#[test]
fn the_invite_save_writes_a_new_key_only_when_given() {
    use crate::config::config_file::{
        self, CliOverrides, ConfigSource, LoadedConfigFile, PathRule, SecretRead,
    };
    use crate::transport::net::NodeSecret;
    for new in [Some(NodeSecret::from_bytes(&[77u8; 32])), None] {
        // given: a slot with a settings file
        let dir = std::env::temp_dir().join(format!(
            "mesh-invite-save-{}-{}",
            std::process::id(),
            new.is_some()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = ConfigSource::File {
            path: dir.join("config.json"),
            may_create: true,
            rule: PathRule::Explicit,
        };
        let run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
        let old = run.secret.peer_id();
        let config = std::sync::Arc::new(std::sync::Mutex::new(
            crate::web_gui::ConfigState::from_run(&run),
        ));
        let key = crate::members::network_key::test_net("x").text;
        // when
        let save = super::save_key_of(config.clone(), false);
        save(&key, new.as_ref()).expect("the key is saved");
        // then: the network key is written; the node key is the new one only when it was given
        let LoadedConfigFile::Parsed(p) = config_file::load_config_file(&dir.join("config.json"))
        else {
            panic!("the settings file must parse");
        };
        assert_eq!(p.settings.net.network_key.as_deref(), Some(key.as_str()));
        let SecretRead::Valid(stored) = p.secret else {
            panic!("the file must hold a secret")
        };
        let want = new.as_ref().map(NodeSecret::peer_id).unwrap_or(old);
        assert_eq!(stored.peer_id(), want, "new key given: {}", new.is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
