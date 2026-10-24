//! `POST /api/network/join` without a slot in the address, through the real router over a `FakeHost`:
//! the status and the slot the join went to.
// helper: mounted from `join_target.rs`; the fixture is in `join_target_tests.rs`.

use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

use super::tests::{fx, Fx};
use crate::config::config_file::NetSlotId;
use crate::members::network_key::NetKind;
use crate::web_gui::control::{ActionError, InviteJoinState, InviteJoinView};
use crate::web_gui::routes::invite_fixture::{invite_into, stale_invite_into, word_key};
use crate::web_gui::test_host::{send, test_app, HostCall};
use crate::web_gui::{AddedSlot, HostError};

const X: &str = "ab2c3d";

fn added(fresh: bool) -> Result<AddedSlot, HostError> {
    Ok(AddedSlot {
        id: NetSlotId::parse(X).unwrap(),
        fresh,
    })
}

async fn join(f: &Fx, path: &str, text: &str) -> (StatusCode, Value) {
    let router = test_app(f.host.clone()).0;
    let body = json!({"key": text}).to_string();
    send(&router, Method::POST, path, &body).await
}

async fn join_auto(f: &Fx, text: &str) -> (StatusCode, Value) {
    join(f, "/api/network/join", text).await
}

fn joins(f: &Fx) -> Vec<(String, usize)> {
    f.nodes
        .iter()
        .map(|(id, n)| (id.clone(), n.join_calls()))
        .collect()
}

fn joined_only(f: &Fx, id: &str) {
    let want: Vec<(String, usize)> = f
        .nodes
        .iter()
        .map(|(n, _)| (n.clone(), usize::from(n == id)))
        .collect();
    assert_eq!(joins(f), want);
}

fn nothing_joined(f: &Fx) {
    assert!(joins(f).iter().all(|(_, n)| *n == 0), "{:?}", joins(f));
}

fn no_slot_taken(f: &Fx) {
    assert!(
        !f.host.calls().contains(&HostCall::Add),
        "{:?}",
        f.host.calls()
    );
}

mod an_invite_on_the_unnamed_address {
    use super::*;

    mod of_a_new_network {
        use super::*;

        #[tokio::test]
        async fn with_main_empty_goes_to_main_and_takes_no_slot() {
            let f = fx(&[("main", None)]);
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], "main", "{body}");
            joined_only(&f, "main");
            no_slot_taken(&f);
        }

        #[tokio::test]
        async fn with_main_busy_takes_a_new_slot_and_joins_it() {
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
            f.host.set_add(added(true));
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], X, "{body}");
            joined_only(&f, X);
            assert!(f.host.calls().contains(&HostCall::Add));
        }

        #[tokio::test]
        async fn with_main_waiting_takes_the_new_invite_into_main() {
            let f = fx(&[("main", None)]);
            *f.node("main").invite_join.lock().unwrap() = Some(InviteJoinView {
                state: InviteJoinState::Waiting,
                message: "waiting".to_string(),
                label: Some("older".to_string()),
                expires_unix: 0,
            });
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], "main", "{body}");
            joined_only(&f, "main");
            no_slot_taken(&f);
        }

        #[tokio::test]
        async fn with_an_empty_slot_handed_back_joins_it() {
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
            f.host.set_add(added(false));
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], X, "{body}");
            joined_only(&f, X);
            assert!(!f
                .host
                .calls()
                .iter()
                .any(|c| matches!(c, HostCall::Retire(..))));
        }

        #[tokio::test]
        async fn of_a_managed_network_takes_a_new_slot_too() {
            let root = crate::transport::net::NodeSecret::from_bytes(&[7; 32]).peer_id();
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
            f.host.set_add(added(true));
            let invite = invite_into(&word_key("beta"), NetKind::Managed { root });
            let (status, body) = join_auto(&f, &invite).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], X, "{body}");
            joined_only(&f, X);
        }
    }

    mod of_a_network_already_here {
        use super::*;

        #[tokio::test]
        async fn of_an_active_network_names_its_slot_and_joins_nothing() {
            let b = word_key("beta");
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, Some(&b))]);
            let (status, body) = join_auto(&f, &invite_into(&b, NetKind::Peer)).await;
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(body["error"], "net_already_added", "{body}");
            assert_eq!(body["slot"], X, "{body}");
            assert_eq!(body["label"], "beta", "{body}");
            nothing_joined(&f);
            no_slot_taken(&f);
        }

        #[tokio::test]
        async fn of_a_network_where_this_node_was_removed_returns_to_its_slot() {
            let b = word_key("beta");
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, Some(&b))]);
            f.remove(X);
            let (status, body) = join_auto(&f, &invite_into(&b, NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], X, "{body}");
            joined_only(&f, X);
            no_slot_taken(&f);
        }

        #[tokio::test]
        async fn of_a_network_where_this_node_is_excluded_returns_to_its_slot() {
            let b = word_key("beta");
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, Some(&b))]);
            f.exclude(X);
            let (status, body) = join_auto(&f, &invite_into(&b, NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], X, "{body}");
            joined_only(&f, X);
            no_slot_taken(&f);
        }

        #[tokio::test]
        async fn of_a_network_held_by_main_where_removed_returns_to_main() {
            let b = word_key("beta");
            let f = fx(&[("main", Some(&b)), (X, Some(&word_key("alpha")))]);
            f.remove("main");
            let (status, body) = join_auto(&f, &invite_into(&b, NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], "main", "{body}");
            joined_only(&f, "main");
        }
    }

    mod at_the_limit {
        use super::*;

        #[tokio::test]
        async fn at_the_limit_is_refused_with_too_many_nets_and_joins_nothing() {
            let f = fx(&[("main", Some(&word_key("alpha")))]);
            f.host.set_add(Err(HostError::TooManyNets));
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(body["error"], "too_many_nets", "{body}");
            assert!(body["limit"].is_number(), "{body}");
            nothing_joined(&f);
        }

        #[tokio::test]
        async fn at_the_limit_a_return_still_joins() {
            let b = word_key("beta");
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, Some(&b))]);
            f.host.set_add(Err(HostError::TooManyNets));
            f.remove(X);
            let (status, body) = join_auto(&f, &invite_into(&b, NetKind::Peer)).await;
            assert_eq!(status, StatusCode::ACCEPTED, "{body}");
            assert_eq!(body["slot"], X, "{body}");
            joined_only(&f, X);
        }

        #[tokio::test]
        async fn at_the_limit_an_active_network_still_names_its_slot() {
            let b = word_key("beta");
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, Some(&b))]);
            f.host.set_add(Err(HostError::TooManyNets));
            let (status, body) = join_auto(&f, &invite_into(&b, NetKind::Peer)).await;
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            assert_eq!(body["error"], "net_already_added", "{body}");
            assert_eq!(body["slot"], X, "{body}");
        }
    }

    mod when_the_node_refuses {
        use super::*;

        fn refuse(f: &Fx, id: &str) {
            f.node(id)
                .set_join_result(Err(ActionError::Busy("busy".to_string())));
        }

        #[tokio::test]
        async fn refused_by_the_node_gives_the_new_slot_back() {
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
            f.host.set_add(added(true));
            refuse(&f, X);
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert_eq!(status, StatusCode::CONFLICT, "{body}");
            let give_back = HostCall::Retire(NetSlotId::parse(X).unwrap(), false);
            assert!(f.host.calls().contains(&give_back), "{:?}", f.host.calls());
        }

        #[tokio::test]
        async fn refused_by_the_node_keeps_a_handed_back_slot() {
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
            f.host.set_add(added(false));
            refuse(&f, X);
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert!(!status.is_success(), "{status} {body}");
            assert!(!f
                .host
                .calls()
                .iter()
                .any(|c| matches!(c, HostCall::Retire(..))));
        }

        #[tokio::test]
        async fn refused_by_the_node_in_main_retires_nothing() {
            let f = fx(&[("main", None)]);
            refuse(&f, "main");
            let (status, body) =
                join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
            assert!(!status.is_success(), "{status} {body}");
            assert_eq!(f.host.calls(), vec![]);
        }
    }

    mod before_any_slot {
        use super::*;

        #[tokio::test]
        async fn expired_is_refused_before_any_slot_is_taken() {
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
            f.host.set_add(added(true));
            let (status, body) = join_auto(&f, &stale_invite_into(&word_key("beta"))).await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
            assert_eq!(body["error"], "invalid_key", "{body}");
            no_slot_taken(&f);
            nothing_joined(&f);
        }

        #[tokio::test]
        async fn broken_is_refused_before_any_slot_is_taken() {
            let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
            f.host.set_add(added(true));
            let mut invite = invite_into(&word_key("beta"), NetKind::Peer);
            invite.truncate(invite.len() - 6);
            let (status, body) = join_auto(&f, &invite).await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
            no_slot_taken(&f);
            nothing_joined(&f);
        }
    }

    #[tokio::test]
    async fn the_answer_names_the_slot() {
        let f = fx(&[("main", None)]);
        let (status, body) = join_auto(&f, &invite_into(&word_key("beta"), NetKind::Peer)).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        assert!(body["slot"].is_string(), "{body}");
    }
}

mod a_word_key_on_the_unnamed_address {
    use super::*;

    #[tokio::test]
    async fn a_word_key_goes_to_main_as_before() {
        let f = fx(&[("main", None)]);
        let (status, body) = join_auto(&f, &word_key("w")).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        assert_eq!(body["slot"], "main", "{body}");
        assert_eq!(body["restart"], true, "{body}");
    }

    #[tokio::test]
    async fn a_word_key_of_another_network_on_a_busy_main_is_refused_as_before() {
        let f = fx(&[("main", Some(&word_key("alpha")))]);
        let (status, body) = join_auto(&f, &word_key("beta")).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"], "network_present", "{body}");
        no_slot_taken(&f);
    }
}

mod an_invite_on_a_slot_address {
    use super::*;

    #[tokio::test]
    async fn a_slot_address_keeps_the_strict_check() {
        let f = fx(&[("main", None), (X, Some(&word_key("alpha")))]);
        let invite = invite_into(&word_key("beta"), NetKind::Peer);
        let (status, body) = join(&f, &format!("/api/net/{X}/network/join"), &invite).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"], "network_present", "{body}");
        nothing_joined(&f);
    }

    #[tokio::test]
    async fn the_main_slot_address_keeps_the_strict_check() {
        let f = fx(&[("main", Some(&word_key("alpha"))), (X, None)]);
        let invite = invite_into(&word_key("beta"), NetKind::Peer);
        let (status, body) = join(&f, "/api/net/main/network/join", &invite).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"], "network_present", "{body}");
        nothing_joined(&f);
    }
}
