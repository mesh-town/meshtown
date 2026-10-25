//! Tests for the SOCKS5 server's I/O: the greeting and the request over a duplex (`wire.rs`).

use super::*;
use crate::proxy::socks5::{encode_socks5_request, Target, REP_OK};

#[tokio::test]
async fn greeting_and_request_pass_through_duplex() {
    let (mut client, mut server) = tokio::io::duplex(1024);
    let client_side = crate::transport::net::task_label::spawn(async move {
        client
            .write_all(&[VERSION, 1, METHOD_NO_AUTH])
            .await
            .unwrap();
        let mut answer = [0u8; 2];
        client.read_exact(&mut answer).await.unwrap();
        assert_eq!(answer, [VERSION, METHOD_NO_AUTH]);
        let raw = encode_socks5_request(&Target::Domain("example.com".to_string()), 443).unwrap();
        client.write_all(&raw).await.unwrap();
        let mut reply = [0u8; 10];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply[1], REP_OK);
    });
    negotiate_auth_method(&mut server).await.unwrap();
    let raw = read_request_bytes(&mut server).await.unwrap();
    let req = crate::proxy::socks5::parse_socks5_request(&raw).unwrap();
    assert_eq!(req.target, Target::Domain("example.com".to_string()));
    write_reply(&mut server, REP_OK).await.unwrap();
    client_side.await.unwrap();
}
