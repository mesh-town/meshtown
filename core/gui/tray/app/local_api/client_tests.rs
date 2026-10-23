use super::*;
use crate::transport::net::NodeSecret;

use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use crate::gui::tray::app::local_api::tests::{
    start_fake, start_fake_with_config, temp_dir, FAKE_TOKEN,
};

// ==============================================================================================
// web_* — against start_fake (5)
// ==============================================================================================

#[tokio::test(flavor = "multi_thread")]
async fn web_valid_token_get_state_is_ok_online() {
    let dir = temp_dir("web-ok");
    let (handle, _node) = start_fake(0, &dir).await;
    let port = handle.shared.port();

    let reply = get_state(port, FAKE_TOKEN)
        .await
        .expect("get_state: the exchange must not fail");
    match reply {
        Reply::Ok(value) => assert_eq!(value["node"]["status"], "online"),
        other => panic!("expected Reply::Ok, got {other:?}"),
    }

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn web_wrong_token_is_unauthorized() {
    let dir = temp_dir("web-401");
    let (handle, _node) = start_fake(0, &dir).await;
    let port = handle.shared.port();

    let wrong_token = "0000000000000000000000000000000000000000000000000A";
    let reply = get_state(port, wrong_token)
        .await
        .expect("get_state: the exchange must not fail");
    assert!(matches!(reply, Reply::Unauthorized));

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn web_post_stop_is_accepted_and_stop_called() {
    let dir = temp_dir("web-stop");
    let (handle, node) = start_fake(0, &dir).await;
    let port = handle.shared.port();

    let reply = post_stop(port, FAKE_TOKEN)
        .await
        .expect("post_stop: the exchange must not fail");
    assert!(matches!(reply, Reply::Accepted));
    assert!(node.stop_called());

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn web_second_post_stop_is_stopping() {
    let dir = temp_dir("web-stop2");
    let (handle, _node) = start_fake(0, &dir).await;
    let port = handle.shared.port();

    let first = post_stop(port, FAKE_TOKEN).await.expect("first post_stop");
    assert!(matches!(first, Reply::Accepted));
    let second = post_stop(port, FAKE_TOKEN).await.expect("second post_stop");
    assert!(matches!(second, Reply::Stopping));

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn web_after_shutdown_is_refused() {
    let dir = temp_dir("web-gone");
    let (handle, _node) = start_fake(0, &dir).await;
    let port = handle.shared.port();

    handle.shutdown(Duration::from_secs(2)).await;

    let err = get_state(port, FAKE_TOKEN)
        .await
        .expect_err("after shutdown the port is free, the exchange must fail");
    assert!(matches!(err, ApiError::Refused));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn web_get_config_returns_current_document() {
    let dir = temp_dir("web-config-get");
    let (handle, _node) = start_fake(0, &dir).await;
    let port = handle.shared.port();

    let reply = get_config(port, FAKE_TOKEN)
        .await
        .expect("get_config: the exchange must not fail");
    match reply {
        Reply::Ok(value) => assert!(
            value.get("rev").is_some(),
            "the response contains rev: {value}"
        ),
        other => panic!("expected Reply::Ok, got {other:?}"),
    }

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn web_put_config_round_trips_route_exits() {
    let dir = temp_dir("web-config-put");
    let (handle, _node) = start_fake_with_config(0, &dir).await;
    let port = handle.shared.port();

    let current = get_config(port, FAKE_TOKEN)
        .await
        .expect("get_config: the exchange must not fail");
    let rev = match current {
        Reply::Ok(value) => value["rev"].as_u64().expect("rev is a number"),
        other => panic!("expected Reply::Ok, got {other:?}"),
    };
    // `route.exits` is parsed by the server as a list of `PeerId` (`take_endpoint_id_list`,
    // `config_file/parse_fields.rs`), not arbitrary strings — otherwise the whole list is silently
    // rolled back to `[]`. A real id is made the same way as in `poller_exit_tests.rs::id`.
    let exit_id = NodeSecret::from_bytes(&[7u8; 32]).peer_id().to_string();
    let body = serde_json::json!({"rev": rev, "config": {"route": {"exits": [exit_id.clone()]}}})
        .to_string()
        .into_bytes();

    let reply = put_config(port, FAKE_TOKEN, &body)
        .await
        .expect("put_config: the exchange must not fail");
    assert!(
        matches!(reply, Reply::Ok(_)),
        "a PUT of a valid body with the right rev must return 200, got {reply:?}"
    );

    // The loop closes only with a repeated GET: a PUT returns 200 even for a rejected field (with
    // a wrong value the server rolls the list back to [] and still answers `Ok`),
    // so what actually landed on disk has to be checked.
    let after = get_config(port, FAKE_TOKEN)
        .await
        .expect("get_config after PUT: the exchange must not fail");
    match after {
        Reply::Ok(value) => {
            let exits = value["config"]["route"]["exits"]
                .as_array()
                .expect("config.route.exits is a list");
            assert_eq!(
                exits,
                &vec![serde_json::Value::String(exit_id)],
                "route.exits after the PUT must contain the written id: {value}"
            );
        }
        other => panic!("expected Reply::Ok, got {other:?}"),
    }

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

// ==============================================================================================
// raw_* — against our own TcpListener (3)
// ==============================================================================================

#[tokio::test(flavor = "multi_thread")]
async fn raw_listener_silent_is_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    let accept_task = crate::node::output::spawn(async move {
        let (_socket, _addr) = listener.accept().await.expect("accept");
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let started = Instant::now();
    let err = get_state(port, FAKE_TOKEN)
        .await
        .expect_err("a silent listener must time out");
    assert!(matches!(err, ApiError::Timeout));
    assert!(started.elapsed() <= Duration::from_secs(3));

    accept_task.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn raw_listener_ssh_banner_is_not_http() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    let accept_task = crate::node::output::spawn(async move {
        let (mut socket, _addr) = listener.accept().await.expect("accept");
        let _ = socket.write_all(b"SSH-2.0-x\r\n\r\n").await;
    });

    let err = get_state(port, FAKE_TOKEN)
        .await
        .expect_err("an SSH banner is not an HTTP response");
    assert!(matches!(err, ApiError::NotHttp(_)));

    accept_task.await.expect("the stub server must not panic");
}

#[tokio::test(flavor = "multi_thread")]
async fn raw_port_closed_is_refused() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();
    drop(listener);

    let err = get_state(port, FAKE_TOKEN)
        .await
        .expect_err("the port is free — the connection must be refused");
    assert!(matches!(err, ApiError::Refused));
}

// ==============================================================================================
// get_invite — against start_fake (3)
// ==============================================================================================

#[tokio::test(flavor = "multi_thread")]
async fn get_invite_returns_invite_with_token() {
    let dir = temp_dir("invite-ok");
    let (handle, node) = start_fake(0, &dir).await;
    node.set_invite(Some("mesh-invite2-FAKE"));
    let port = handle.shared.port();

    let reply = get_invite(port, FAKE_TOKEN)
        .await
        .expect("get_invite: the exchange must not fail");
    match reply {
        Reply::Ok(value) => assert_eq!(value["invite"], "mesh-invite2-FAKE"),
        other => panic!(
            "expected Reply::Ok, got a different reply ({})",
            tag(&other)
        ),
    }

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// The kind of a reply without the body: the reply itself stays out of the message (it may hold the invite).
fn tag(reply: &Reply) -> &'static str {
    match reply {
        Reply::Ok(_) => "200",
        Reply::Accepted => "202",
        Reply::Unauthorized => "401",
        Reply::Stopping => "503",
        Reply::OtherStatus(_) => "other code",
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn get_invite_without_token_is_unauthorized() {
    let dir = temp_dir("invite-401");
    let (handle, node) = start_fake(0, &dir).await;
    node.set_invite(Some("mesh-invite2-FAKE"));
    let port = handle.shared.port();

    let reply = get_invite(port, "")
        .await
        .expect("get_invite: the exchange must not fail");
    assert!(matches!(reply, Reply::Unauthorized));

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_invite_busy_is_409() {
    let dir = temp_dir("invite-409");
    let (handle, node) = start_fake(0, &dir).await;
    node.set_invite(None);
    let port = handle.shared.port();

    let reply = get_invite(port, FAKE_TOKEN)
        .await
        .expect("get_invite: the exchange must not fail");
    assert!(matches!(reply, Reply::OtherStatus(409)));

    handle.shutdown(Duration::from_secs(2)).await;
    let _ = std::fs::remove_dir_all(&dir);
}
