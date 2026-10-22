//! Frames produced by the reference udpgw implementation (tun2proxy 0.8.6, MIT); regenerate with its `Packet`
//! builders. They pin the wire format against the code real clients use.

use super::codec::{encode, parse_body, Frame, Target};
use super::session::Limits;
use super::test_support::{cfg_with, spawn_echo, start};
use std::collections::HashMap;
use std::time::Duration;

const REQ_KEEPALIVE_7: &str = "0003010007";
const REQ_DATA_V4: &str = "000e02000101c000020a003570696e67";
const REQ_DATA_DOMAIN: &str = "0014020002030b6578616d706c652e636f6d007b0102";
const REQ_DATA_V6: &str = "00180200030420010db800000000000000000000000100357636";
const REQ_CLOSE_1: &str = "0003040001";
const REQ_UNKNOWN_FLAG_9: &str = "0003080009";
const REQ_ERR_4: &str = "0003200004";
const RESP_KEEPALIVE_7: &str = "0003010007";
const RESP_DATA_V4: &str = "000e02000101c000020a0035706f6e67";
const RESP_ERR_9: &str = "0003200009";

const PLAIN: Limits = Limits {
    idle: Duration::from_secs(60),
    max_sessions: 256,
};

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
        .collect()
}

fn parsed(hex: &str) -> Frame {
    parse_body(&bytes(hex)[2..]).expect("reference frame parses")
}

#[test]
fn reference_requests_parse_into_the_expected_frames() {
    let v4 = Target::Ip("192.0.2.10:53".parse().expect("addr"));
    let v6 = Target::Ip("[2001:db8::1]:53".parse().expect("addr"));
    let domain = Target::Domain("example.com".into(), 123);
    let data = |conn_id, target, data: &[u8]| Frame {
        flags: 0x02,
        conn_id,
        target: Some(target),
        data: data.to_vec(),
    };
    assert_eq!(parsed(REQ_KEEPALIVE_7), Frame::keepalive(7));
    assert_eq!(parsed(REQ_DATA_V4), data(1, v4, b"ping"));
    assert_eq!(parsed(REQ_DATA_DOMAIN), data(2, domain, &[1, 2]));
    assert_eq!(parsed(REQ_DATA_V6), data(3, v6, b"v6"));
    assert_eq!(parsed(REQ_CLOSE_1).flags, 0x04);
}

#[test]
fn our_frames_are_byte_identical_to_the_reference() {
    let from = "192.0.2.10:53".parse().expect("addr");
    assert_eq!(
        encode(&Frame::keepalive(7)).expect("fits"),
        bytes(RESP_KEEPALIVE_7)
    );
    assert_eq!(
        encode(&Frame::data(1, from, b"pong".to_vec())).expect("fits"),
        bytes(RESP_DATA_V4)
    );
    assert_eq!(encode(&Frame::error(9)).expect("fits"), bytes(RESP_ERR_9));
    assert_eq!(encode(&Frame::error(4)).expect("fits"), bytes(REQ_ERR_4));
}

#[tokio::test]
async fn reference_keepalive_gets_the_reference_answer() {
    let (mut client, _task) = start(cfg_with(HashMap::new()), PLAIN);
    client.send_raw(&bytes(REQ_KEEPALIVE_7)).await;
    assert_eq!(client.next_raw().await, bytes(RESP_KEEPALIVE_7));
}

#[tokio::test]
async fn reference_unknown_flag_and_error_frames_get_the_reference_error() {
    let (mut client, _task) = start(cfg_with(HashMap::new()), PLAIN);
    client.send_raw(&bytes(REQ_UNKNOWN_FLAG_9)).await;
    assert_eq!(client.next_raw().await, bytes(RESP_ERR_9));
    client.send_raw(&bytes(REQ_ERR_4)).await;
    assert_eq!(client.next_raw().await, bytes(REQ_ERR_4));
}

#[tokio::test]
async fn reference_domain_datagram_is_echoed_in_the_reference_layout() {
    let echo = spawn_echo().await;
    let cfg = cfg_with(HashMap::from([("example.com".to_string(), echo)]));
    let (mut client, _task) = start(cfg, PLAIN);
    client.send_raw(&bytes(REQ_DATA_DOMAIN)).await;
    let expected = encode(&Frame::data(2, echo, vec![1, 2])).expect("fits");
    assert_eq!(client.next_raw().await, expected);
}

#[tokio::test]
async fn reference_close_ends_the_session_and_keeps_the_stream() {
    let (mut client, _task) = start(cfg_with(HashMap::new()), PLAIN);
    client.send_raw(&bytes(REQ_CLOSE_1)).await;
    client.send_raw(&bytes(REQ_KEEPALIVE_7)).await;
    assert_eq!(client.next_raw().await, bytes(RESP_KEEPALIVE_7));
}
