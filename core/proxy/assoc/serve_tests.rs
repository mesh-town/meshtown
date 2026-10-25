use super::*;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

#[test]
fn no_free_slot_refuses() {
    let slots = Arc::new(Semaphore::new(0));
    assert!(take_slot(&slots).is_none());
}

#[test]
fn a_slot_is_returned_when_the_permit_drops() {
    let slots = Arc::new(Semaphore::new(1));
    let permit = take_slot(&slots).expect("one free slot");
    assert!(take_slot(&slots).is_none());
    drop(permit);
    assert!(take_slot(&slots).is_some());
}

#[test]
fn node_slots_hold_the_limit() {
    assert_eq!(udp_slots().available_permits(), MAX_UDP_ASSOCIATIONS);
}

/// A `TcpStream` pair on loopback: the end accepted by the server and the client's end.
async fn loopback_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (accepted, _) = listener.accept().await.unwrap();
    (accepted, client)
}

/// Sends a refusal with the given `no_udp` and returns the reply code the client reads.
async fn reply_code_for(no_udp: bool) -> u8 {
    let (server, mut client) = loopback_pair().await;
    let refusal = UdpRefusal {
        outcome: ProxyOutcome::refused("no route", "refused"),
        no_udp,
    };

    refuse(server, refusal, Instant::now()).await.unwrap();

    let mut reply = [0u8; 10];
    client.read_exact(&mut reply).await.unwrap();
    reply[1]
}

/// given: the exits have no UDP gateway; then the client gets "command not supported" (7).
#[tokio::test]
async fn gateway_refusal_replies_command_not_supported() {
    assert_eq!(reply_code_for(true).await, socks5::REP_CMD_NOT_SUPPORTED);
}

/// given: any other refusal; then the client gets "general failure" (1).
#[tokio::test]
async fn other_refusal_replies_general_failure() {
    assert_eq!(reply_code_for(false).await, socks5::REP_GENERAL);
}
