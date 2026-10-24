//! Creating a network on a slot (`POST /api/network/create`): the `kind` field and where the creator of a
//! Managed network comes from.

// helper: the tests mount this file next to `network_present_tests.rs`, e.g.
// `#[cfg(test)] #[path = "network_create_tests.rs"] mod create_tests;` in `network.rs`.

use std::path::PathBuf;

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use crate::config::config_file::{
    self, load_config_file, CliOverrides, ConfigSource, LoadedConfigFile, PathRule, SecretRead,
};
use crate::members::network_key::{parse_user_input, NetKind};
use crate::transport::net::{NodeSecret, PeerId};
use crate::web_gui::test_host::{config_with_key, send, slot_handle, test_app, FakeHost, SnapNode};
use crate::web_gui::{ConfigState, SlotHandle};

/// Node secret of the primary slot.
const MAIN_SEED: u8 = 3;
/// Node secret of the slot that was added afterwards.
const ADDED_SEED: u8 = 4;

mod create_managed_root_is_slot_secret {
    use super::*;

    #[tokio::test]
    async fn without_a_file_the_creator_is_the_node_id_of_the_slot_secret() {
        // given
        let f = setup(Opts::default());
        // when
        let (status, reply) = f
            .when_create(
                "/api/network/create",
                json!({"label": "gamma", "kind": "Managed"}),
            )
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(
            f.then_kind_of(&reply),
            NetKind::Managed {
                root: root_of(MAIN_SEED)
            }
        );
    }

    #[tokio::test]
    async fn with_a_file_the_creator_is_also_the_secret_stored_in_the_file() {
        // given
        let f = setup(Opts {
            with_file: true,
            ..Opts::default()
        });
        // when
        let (status, reply) = f
            .when_create(
                "/api/network/create",
                json!({"label": "gamma", "kind": "Managed"}),
            )
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(
            f.then_kind_of(&reply),
            NetKind::Managed {
                root: root_of(MAIN_SEED)
            }
        );
        let stored = NodeSecret::from_bytes(&f.then_file_secret());
        assert_eq!(
            stored.peer_id(),
            root_of(MAIN_SEED),
            "the file holds the secret of the creator"
        );
    }

    #[tokio::test]
    async fn the_slot_keeps_the_key_it_returned() {
        // given
        let f = setup(Opts::default());
        // when
        let (_, reply) = f
            .when_create(
                "/api/network/create",
                json!({"label": "gamma", "kind": "Managed"}),
            )
            .await;
        // then
        assert_eq!(
            f.key_of(&f.main),
            Some(reply["key"].as_str().unwrap().to_string())
        );
    }
}

mod create_managed_on_added_slot {
    use super::*;

    #[tokio::test]
    async fn the_creator_is_the_secret_of_the_added_slot_not_of_the_primary_one() {
        // given: the primary slot works with a network, the added one is empty and has its own secret
        let f = setup(Opts {
            main_network: Some("alpha--abacus-abdomen-0123456789abcdefghjkm".to_string()),
            ..Opts::default()
        });
        // when
        let (status, reply) = f
            .when_create(
                "/api/net/ab2c3d/network/create",
                json!({"label": "gamma", "kind": "Managed"}),
            )
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        let kind = f.then_kind_of(&reply);
        assert_eq!(
            kind,
            NetKind::Managed {
                root: root_of(ADDED_SEED)
            }
        );
        assert_ne!(kind.root(), Some(root_of(MAIN_SEED)));
        assert_eq!(
            f.key_of(&f.added),
            Some(reply["key"].as_str().unwrap().to_string())
        );
    }

    #[tokio::test]
    async fn the_primary_slot_keeps_its_network() {
        // given
        let key = "alpha--abacus-abdomen-0123456789abcdefghjkm".to_string();
        let f = setup(Opts {
            main_network: Some(key.clone()),
            ..Opts::default()
        });
        // when
        f.when_create(
            "/api/net/ab2c3d/network/create",
            json!({"label": "gamma", "kind": "Managed"}),
        )
        .await;
        // then
        assert_eq!(f.key_of(&f.main), Some(key));
    }
}

mod create_without_kind_is_peer {
    use super::*;

    #[tokio::test]
    async fn the_key_is_a_peer_key_of_the_old_shape() {
        // given
        let f = setup(Opts::default());
        // when
        let (status, reply) = f
            .when_create("/api/network/create", json!({"label": "gamma"}))
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(f.then_kind_of(&reply), NetKind::Peer);
        assert_eq!(
            random_part_count(&reply),
            3,
            "label--word-word-group: {reply}"
        );
    }
}

mod create_kind_peer {
    use super::*;

    #[tokio::test]
    async fn the_key_is_a_peer_key_of_the_old_shape() {
        // given
        let f = setup(Opts::default());
        // when
        let (status, reply) = f
            .when_create(
                "/api/network/create",
                json!({"label": "gamma", "kind": "Peer"}),
            )
            .await;
        // then
        assert_eq!(status, StatusCode::OK, "{reply}");
        assert_eq!(f.then_kind_of(&reply), NetKind::Peer);
        assert_eq!(
            random_part_count(&reply),
            3,
            "label--word-word-group: {reply}"
        );
    }
}

mod create_bad_kind {
    use super::*;

    #[tokio::test]
    async fn a_value_that_is_not_a_kind_is_refused_on_the_kind_field() {
        for kind in [json!("bogus"), json!(5), Value::Null] {
            // given
            let f = setup(Opts::default());
            // when
            let (status, reply) = f
                .when_create(
                    "/api/network/create",
                    json!({"label": "gamma", "kind": kind}),
                )
                .await;
            // then
            assert_eq!(
                status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "kind {kind}: {reply}"
            );
            assert_eq!(reply["error"], "invalid", "kind {kind}");
            let fields = reply["fields"].as_array().expect("fields is a list");
            assert_eq!(fields.len(), 1, "kind {kind}: {reply}");
            assert_eq!(fields[0]["path"], "kind", "kind {kind}");
            assert!(
                fields[0]["message"].as_str().is_some_and(|m| !m.is_empty()),
                "kind {kind}: the field has a message: {reply}"
            );
        }
    }

    #[tokio::test]
    async fn nothing_is_written_to_the_slot() {
        // given
        let f = setup(Opts::default());
        // when
        f.when_create(
            "/api/network/create",
            json!({"label": "gamma", "kind": "bogus"}),
        )
        .await;
        // then
        assert_eq!(f.key_of(&f.main), None);
    }
}

#[derive(Default)]
struct Opts {
    /// The primary slot has a settings file in a temporary directory.
    with_file: bool,
    /// The network the primary slot already works with.
    main_network: Option<String>,
}

struct Fixture {
    router: axum::Router,
    main: SlotHandle,
    added: SlotHandle,
    dir: Option<PathBuf>,
}

/// The primary slot `main` (secret `MAIN_SEED`) and the empty slot `ab2c3d` (secret `ADDED_SEED`).
fn setup(opts: Opts) -> Fixture {
    let dir = opts.with_file.then(temp_dir);
    let mut state = match &dir {
        Some(dir) => file_state(dir),
        None => config_with_key(None),
    };
    state.secret = NodeSecret::from_bytes(&[MAIN_SEED; 32]);
    state.current_network_key = opts.main_network;
    let main = slot_handle("main", SnapNode::new(Default::default()), state);
    let mut empty = config_with_key(None);
    empty.secret = NodeSecret::from_bytes(&[ADDED_SEED; 32]);
    // helper: the fake host cannot add slots itself, so the slot a real `POST /api/nets` would add is present from the start.
    let added = slot_handle("ab2c3d", SnapNode::new(Default::default()), empty);
    let host = FakeHost::new(vec![main.clone(), added.clone()]);
    Fixture {
        router: test_app(host).0,
        main,
        added,
        dir,
    }
}

impl Fixture {
    async fn when_create(&self, path: &str, body: Value) -> (StatusCode, Value) {
        send(&self.router, Method::POST, path, &body.to_string()).await
    }

    /// The kind of the key the response carries.
    fn then_kind_of(&self, reply: &Value) -> NetKind {
        let key = reply["key"].as_str().expect("the response has a key");
        parse_user_input(key).expect("the returned key parses").kind
    }

    fn key_of(&self, slot: &SlotHandle) -> Option<String> {
        slot.config.lock().unwrap().current_network_key.clone()
    }

    /// `identity.secret` of the primary slot's file.
    fn then_file_secret(&self) -> [u8; 32] {
        let path = self
            .dir
            .as_ref()
            .expect("the fixture has a file")
            .join("config.json");
        match load_config_file(&path) {
            LoadedConfigFile::Parsed(p) => match p.secret {
                SecretRead::Valid(secret) => secret.to_bytes(),
                other => panic!("the file must hold a valid secret: {other:?}"),
            },
            other => panic!("the file must be readable: {other:?}"),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn root_of(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

/// Number of hyphen-separated parts right of `--` in the returned key.
fn random_part_count(reply: &Value) -> usize {
    let key = reply["key"].as_str().expect("the response has a key");
    key.split_once("--")
        .expect("the key has a label")
        .1
        .split('-')
        .count()
}

fn file_state(dir: &std::path::Path) -> ConfigState {
    let source = ConfigSource::File {
        path: dir.join("config.json"),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = config_file::prepare(&source, &CliOverrides::default(), true, 0)
        .expect("prepare with a file");
    ConfigState::from_run(&run)
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-create-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
