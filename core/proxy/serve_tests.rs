//! Tests for the proxy's listening socket: a busy port (`serve.rs`).

use super::*;

#[tokio::test]
async fn bind_refuses_busy_port() {
    let first = bind_proxy_listener(0, ListenAccess::Localhost)
        .await
        .unwrap();
    let port = first.local_addr().unwrap().port();
    let e = bind_proxy_listener(port, ListenAccess::Localhost)
        .await
        .unwrap_err();
    let bind_error = e
        .downcast_ref::<BindError>()
        .expect("the error is a BindError");
    assert_eq!(bind_error.port, port);
    assert_eq!(bind_error.io_error_kind, std::io::ErrorKind::AddrInUse);
    drop(first);
}

fn err(port: u16, kind: std::io::ErrorKind) -> BindError {
    BindError {
        port,
        io_error_kind: kind,
        os_message: "OS".into(),
    }
}

#[test]
fn busy_text_and_advice() {
    let e = err(1080, std::io::ErrorKind::AddrInUse);
    assert_eq!(
        e.to_string(),
        "port 1080 for the browser proxy is in use by another program (OS)."
    );
    assert_eq!(
        e.advice(),
        "Close it or start Mesh Town on another port: meshtown run --socks-port 1081"
    );
}

#[test]
fn no_permission_text() {
    let e = err(80, std::io::ErrorKind::PermissionDenied);
    assert_eq!(
        e.to_string(),
        "no permission to open port 80 for the browser proxy (OS)."
    );
    assert_eq!(
        e.advice(),
        "Start Mesh Town on another port: meshtown run --socks-port 81"
    );
}

#[test]
fn other_kind_text() {
    let e = err(1080, std::io::ErrorKind::Other);
    assert_eq!(
        e.to_string(),
        "port 1080 for the browser proxy is in use or unavailable (OS)."
    );
    assert!(e.advice().starts_with("Start Mesh Town on another port"));
}

#[test]
fn port_65535_advice_1_down() {
    let e = err(65535, std::io::ErrorKind::AddrInUse);
    assert!(e.advice().ends_with("--socks-port 65534"));
}

#[tokio::test]
async fn low_latency_sets_nodelay() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (accepted, _) = listener.accept().await.unwrap();
    assert!(!accepted.nodelay().unwrap(), "Nagle is on by default");

    disable_nagle(&accepted);

    assert!(accepted.nodelay().unwrap());
    drop(client);
}

mod client_watch {
    use tokio::io::AsyncWriteExt;
    use tokio::time::timeout;

    use super::*;

    mod peer_closed {
        use super::*;

        #[tokio::test]
        async fn client_gone_resolves_on_close() {
            // given: the peer closed the socket
            let (accepted, peer) = loopback_pair().await;
            drop(peer);

            // when: we wait for `client_gone`
            let finished = timeout(Duration::from_millis(500), client_gone(&accepted)).await;

            // then: it finished within 500 ms
            assert!(
                finished.is_ok(),
                "client_gone did not notice the socket closing within 500 ms"
            );
        }
    }

    mod peer_sent_data {
        use super::*;

        #[tokio::test]
        async fn client_gone_waits_while_client_sent_data() {
            // given: the peer sent a byte and keeps the socket open
            let (accepted, mut peer) = loopback_pair().await;
            peer.write_all(b"x").await.unwrap();

            // when: we wait for `client_gone` for 200 ms
            let finished = timeout(Duration::from_millis(200), client_gone(&accepted)).await;

            // then: it did not finish, and the byte is still visible to `peek` (the data waits for `relay_bytes`)
            assert!(
                finished.is_err(),
                "a client with data in the socket has not \"left\""
            );
            let mut byte = [0u8; 1];
            let seen = accepted.peek(&mut byte).await.unwrap();
            assert_eq!(
                (seen, byte[0]),
                (1, b'x'),
                "client_gone must not take the byte"
            );
        }
    }

    /// A `TcpStream` pair on loopback: the end accepted by the server and the client's end.
    async fn loopback_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (accepted, _) = listener.accept().await.unwrap();
        (accepted, peer)
    }
}
