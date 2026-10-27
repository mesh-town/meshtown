//! A repeated invite goes back to the slot that already waits for it instead of taking another slot.
// helper: mounted from `join_target.rs`; the fixture is in `join_target_tests.rs`.

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use axum::Router;
use serde_json::{json, Value};

use super::tests::{fx, id_hex, Fx};
use crate::config::config_file::NetSlotId;
use crate::web_gui::control::{ActionError, InviteJoinState, InviteJoinView};
use crate::web_gui::routes::invite_fixture::{invite_into, word_key};
use crate::web_gui::routes::pending_invites::remember;
use crate::web_gui::test_host::{send, test_app, HostCall};
use crate::web_gui::{AddedSlot, App};

const X: &str = "ab2c3d";
const Y: &str = "cd4e5f";

fn slot(id: &str) -> NetSlotId {
    NetSlotId::parse(id).unwrap()
}

fn set_add(f: &Fx, id: &str) {
    f.host.set_add(Ok(AddedSlot {
        id: slot(id),
        fresh: true,
    }));
}

fn wait(f: &Fx, id: &str, state: InviteJoinState) {
    *f.node(id).invite_join.lock().unwrap() = Some(InviteJoinView {
        state,
        message: "waiting".to_string(),
        label: None,
        expires_unix: 0,
    });
}

fn adds(f: &Fx) -> usize {
    f.host
        .calls()
        .iter()
        .filter(|c| **c == HostCall::Add)
        .count()
}

fn joins_of(f: &Fx, id: &str) -> usize {
    f.node(id).join_calls()
}

fn noted(app: &App, key_text: &str) -> Option<NetSlotId> {
    let map = app.pending_invites.lock().unwrap();
    map.get(&id_hex(key_text)).cloned()
}

fn start(f: &Fx) -> (Router, Arc<App>) {
    test_app(f.host.clone())
}

async fn invite(router: &Router, key_text: &str) -> (StatusCode, Value) {
    let body = json!({"key": invite_into(key_text)}).to_string();
    send(router, Method::POST, "/api/network/join", &body).await
}

#[tokio::test]
async fn repeat_while_waiting_joins_the_same_slot_and_adds_none() {
    let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
    set_add(&f, X);
    let (router, _) = start(&f);
    let b = word_key("beta");
    assert_eq!(invite(&router, &b).await.0, StatusCode::ACCEPTED);
    wait(&f, X, InviteJoinState::Waiting);
    let (status, body) = invite(&router, &b).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!((adds(&f), joins_of(&f, X)), (1, 2));
}

#[tokio::test]
async fn repeat_while_waiting_in_main_stays_in_main() {
    let f = fx(&[("main", None)]);
    let (router, _) = start(&f);
    let b = word_key("beta");
    assert_eq!(invite(&router, &b).await.0, StatusCode::ACCEPTED);
    wait(&f, "main", InviteJoinState::Waiting);
    assert_eq!(invite(&router, &b).await.0, StatusCode::ACCEPTED);
    assert_eq!((adds(&f), joins_of(&f, "main")), (0, 2));
}

#[tokio::test]
async fn an_invite_to_another_network_takes_its_own_slot() {
    let f = fx(&[("main", Some(&word_key("alpha"))), (X, None), (Y, None)]);
    set_add(&f, X);
    let (router, app) = start(&f);
    let (b, c) = (word_key("beta"), word_key("gamma"));
    invite(&router, &b).await;
    wait(&f, X, InviteJoinState::Waiting);
    set_add(&f, Y);
    let (status, body) = invite(&router, &c).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["slot"], Y, "{body}");
    assert_eq!(noted(&app, &b), Some(slot(X)));
    assert_eq!(noted(&app, &c), Some(slot(Y)));
}

#[tokio::test]
async fn after_the_join_the_repeat_names_the_slot() {
    let b = word_key("beta");
    let f = fx(&[("main", Some(&word_key("alpha"))), (X, Some(&b))]);
    let (router, app) = start(&f);
    remember(&app, &id_hex(&b), slot(X));
    let (status, body) = invite(&router, &b).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"], "net_already_added", "{body}");
    assert_eq!(noted(&app, &b), None);
}

#[tokio::test]
async fn after_a_refusal_the_record_is_forgotten() {
    let f = fx(&[("main", Some(&word_key("alpha"))), (X, None), (Y, None)]);
    wait(&f, X, InviteJoinState::Refused);
    set_add(&f, Y);
    let (router, app) = start(&f);
    let b = word_key("beta");
    remember(&app, &id_hex(&b), slot(X));
    let (_, body) = invite(&router, &b).await;
    assert_eq!(body["slot"], Y, "{body}");
    assert_eq!((joins_of(&f, X), adds(&f)), (0, 1));
    assert_eq!(noted(&app, &b), Some(slot(Y)));
}

#[tokio::test]
async fn a_retired_slot_drops_the_record() {
    let f = fx(&[("main", None)]);
    let (router, app) = start(&f);
    let b = word_key("beta");
    remember(&app, &id_hex(&b), slot(X));
    let (status, body) = invite(&router, &b).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["slot"], "main", "{body}");
    assert_eq!(noted(&app, &b), Some(NetSlotId::main()));
}

#[tokio::test]
async fn a_refused_first_join_records_nothing() {
    let f = fx(&[("main", None)]);
    f.node("main")
        .set_join_result(Err(ActionError::Busy("busy".to_string())));
    let (router, app) = start(&f);
    let b = word_key("beta");
    assert_eq!(invite(&router, &b).await.0, StatusCode::CONFLICT);
    assert_eq!(noted(&app, &b), None);
}

#[tokio::test]
async fn the_answer_to_a_repeat_names_the_same_slot() {
    let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
    set_add(&f, X);
    let (router, _) = start(&f);
    let b = word_key("beta");
    let first = invite(&router, &b).await.1;
    wait(&f, X, InviteJoinState::Waiting);
    let second = invite(&router, &b).await.1;
    assert_eq!((&first["slot"], &second["slot"]), (&json!(X), &json!(X)));
}

#[tokio::test]
async fn a_repeat_is_not_sent_to_a_slot_waiting_for_another_network() {
    const Z: &str = "ef6a7b";
    let f = fx(&[
        ("main", Some(&word_key("alpha"))),
        (X, None),
        (Y, None),
        (Z, None),
    ]);
    wait(&f, X, InviteJoinState::Refused);
    wait(&f, Y, InviteJoinState::Waiting);
    set_add(&f, Z);
    let (router, app) = start(&f);
    let (b, c) = (word_key("beta"), word_key("gamma"));
    remember(&app, &id_hex(&b), slot(X));
    remember(&app, &id_hex(&c), slot(Y));
    let (status, body) = invite(&router, &b).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!((joins_of(&f, X), joins_of(&f, Y)), (0, 0), "{body}");
    assert_eq!(noted(&app, &c), Some(slot(Y)));
}
