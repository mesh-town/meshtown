//! Copy and QR of an open invite from the page's list, over a real node on loopback.
//!
//! What it does: the node hands out an invite the way the page does, the list marks it as kept, the text route
//! returns exactly the issued string (and a QR picture); once a newcomer redeems the invite it leaves the list,
//! the text route answers 404 and the record is gone from the file on disk.
//! What it does not do: the page itself; every wait has a ceiling so a hang fails the test.

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;
#[path = "common/node_web.rs"]
mod web;

use std::time::Duration;

use meshtown_core::invite;
use meshtown_core::members::network_key::NetworkKey;
use serde_json::{json, Value};

use web::{api, boot_consulting_list, teardown, wait_until, Booted};

const LIMIT: Duration = Duration::from_secs(20);
const NEWCOMER_SEED: u8 = 6;

fn call(b: &Booted, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    api(b.port, &b.token, method, path, body)
}

fn open_rows(b: &Booted) -> Vec<Value> {
    let (status, body) = call(b, "GET", "/api/network/invites", None);
    assert_eq!(status, 200, "{body}");
    body["invites"].as_array().cloned().unwrap_or_default()
}

fn text_of(b: &Booted, hash: &str) -> (u16, Value) {
    call(
        b,
        "POST",
        "/api/network/invites/text",
        Some(json!({ "hash": hash })),
    )
}

fn kept_file_mentions(b: &Booted, hash: &str) -> bool {
    let text = std::fs::read_to_string(b.dir.join("kept-invites.json")).unwrap_or_default();
    text.contains(hash)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_issued_invite_can_be_shown_again_until_it_is_used() {
    // given: an invite made through the page's route, with a note
    let b = boot_consulting_list("share-list", &NetworkKey::generate().unwrap()).await;
    let (status, made) = call(&b, "GET", "/api/network/invite?for=Anna", None);
    assert_eq!(status, 200, "{made}");
    let issued = made["invite"].as_str().expect("invite string").to_string();
    // when: the list is read
    let rows = open_rows(&b);
    // then: the row is kept and the text route gives back the issued string
    let row = rows
        .iter()
        .find(|r| r["for_whom"] == "Anna")
        .expect("the row");
    assert_eq!(row["kept"], json!(true), "{row}");
    let hash = row["hash"].as_str().unwrap().to_string();
    let (status, shown) = text_of(&b, &hash);
    assert_eq!(status, 200, "{shown}");
    assert_eq!(shown["invite"].as_str(), Some(issued.as_str()));
    assert!(shown["qr"].is_string(), "the QR picture is drawn");
    assert!(kept_file_mentions(&b, &hash));
    // when: a newcomer redeems it
    let parsed = invite::parse(&issued).expect("parse");
    managed::redeem_as(NEWCOMER_SEED, &parsed.head, &parsed.peers)
        .await
        .expect("the invite must be redeemed");
    // then: it leaves the list, the text is refused and the record leaves the file
    let gone = || !open_rows(&b).iter().any(|r| r["hash"] == hash.as_str());
    assert!(
        wait_until(LIMIT, gone).await,
        "the used invite is still listed"
    );
    let (status, refused) = text_of(&b, &hash);
    assert_eq!(status, 404, "{refused}");
    assert!(!kept_file_mentions(&b, &hash));
    teardown(b, Duration::from_secs(5)).await;
}
