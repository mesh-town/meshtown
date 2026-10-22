//! Tests for `SYNC` admission: the tag is bound to the direction, topic and key; the handshake over a stream.

use super::{accept_handshake, admission_key, dial_handshake, tag, welcome};
use crate::sync::flood::wire::{write_frame, Frame, VERSION};
use crate::net::error::{NetError, NetResult};
use crate::net::facade::test_support::pid;
use crate::net::stream::StreamPair;
use tokio::io::{duplex, split};

const TOPIC: [u8; 32] = [9; 32];

/// The two ends of a stream: what one writes, the other reads.
fn streams() -> (StreamPair, StreamPair) {
    let (a, b) = duplex(1 << 16);
    let (ar, aw) = split(a);
    let (br, bw) = split(b);
    let left = StreamPair {
        send: Box::new(aw),
        recv: Box::new(ar),
    };
    let right = StreamPair {
        send: Box::new(bw),
        recv: Box::new(br),
    };
    (left, right)
}

fn key(seed: u8) -> [u8; 32] {
    admission_key(&[seed; 32])
}

mod tag {
    use super::*;

    /// given: a tag → when: we change the direction, topic or key → then: the tag changes every time.
    #[test]
    fn binds_direction_topic_and_key() {
        let (a, b) = (pid(1), pid(2));
        let base = tag(&key(1), a, b, &TOPIC);

        assert_ne!(base, tag(&key(1), b, a, &TOPIC), "direction");
        assert_ne!(base, tag(&key(1), a, b, &[8; 32]), "topic");
        assert_ne!(base, tag(&key(2), a, b, &TOPIC), "key");
        assert_eq!(base, tag(&key(1), a, b, &TOPIC), "the same triple — the same tag");
    }
}

mod handshake {
    use super::*;

    /// `dialer` (key `kd`) dials, `acceptor` (key `ka`) accepts; the acceptor sends `Welcome` after admission.
    async fn run(
        kd: [u8; 32],
        ka: [u8; 32],
        known: bool,
    ) -> (NetResult<()>, NetResult<[u8; 32]>) {
        let (caller, callee) = (pid(1), pid(2));
        let (mut a, mut b) = streams();
        let dial = dial_handshake(&mut a, &kd, caller, callee, &TOPIC);
        let accept = async move {
            let got = accept_handshake(&mut b, &ka, callee, caller, |_| known).await?;
            write_frame(&mut b.send, &welcome(&ka, callee, caller, &got)).await?;
            Ok(got)
        };
        tokio::join!(dial, accept)
    }

    /// given: a shared seed → when: the handshake → then: the accept returned the topic, the dial is `Ok`.
    #[tokio::test]
    async fn same_seed_admits_both() {
        let (dialed, accepted) = run(key(1), key(1), true).await;

        dialed.expect("dial");
        assert_eq!(accepted.expect("accept"), TOPIC);
    }

    /// given: the sides have different seeds → when: the handshake → then: both `Err`, `Welcome` was not sent.
    #[tokio::test]
    async fn foreign_seed_refused() {
        let (dialed, accepted) = run(key(1), key(2), true).await;

        assert!(accepted.is_err(), "the accept refused");
        assert!(dialed.is_err(), "the dial did not get Welcome");
    }

    /// given: `known` answers "no" → when: the handshake → then: the accept is `Refused`.
    #[tokio::test]
    async fn unknown_topic_refused() {
        let (_, accepted) = run(key(1), key(1), false).await;

        assert!(matches!(accepted, Err(NetError::Refused)));
    }

    /// given: `Hello` with a foreign version and the right tag → when: accept → then: `Refused`.
    #[tokio::test]
    async fn wrong_version_refused() {
        let (caller, callee) = (pid(1), pid(2));
        let (mut a, mut b) = streams();
        let hello = Frame::Hello {
            version: VERSION + 1,
            topic: TOPIC,
            tag: tag(&key(1), caller, callee, &TOPIC),
        };
        write_frame(&mut a.send, &hello).await.expect("write");

        let got = accept_handshake(&mut b, &key(1), callee, caller, |_| true).await;

        assert!(matches!(got, Err(NetError::Refused)));
    }
}

#[test]
fn admission_key_golden() {
    // recompute: `b3sum --derive-key 'meshtown sync admission v1'` over 32 bytes 0x44
    assert_eq!(
        data_encoding::HEXLOWER.encode(&admission_key(&[0x44; 32])),
        "bfd4037d07bd1987447d9f0b3ac060de3a4fef788c4f3b1e91c42d576d7698a8"
    );
}

#[test]
fn context_is_meshtown_v1() {
    assert!(super::CONTEXT.starts_with("meshtown "));
    assert!(super::CONTEXT.ends_with(" v1"));
    assert!(!super::CONTEXT.as_bytes().windows(4).any(|w| w.iter().all(u8::is_ascii_digit)));
}
