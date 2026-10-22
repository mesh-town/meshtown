//! Handshake tests: a "raw" initiator writes the protocol bytes into a `StreamPair` by hand and
//! breaks them (cut off, tail over the limit, silence). Child module of `handshake_peer_tests.rs`:
//! `Fixture`, `setup` and the other helpers are there.

use super::*;
use crate::net::StreamPair;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Handshake tail limit on the wire (bytes) — a number from the format, not from the code under test.
const TAIL_LIMIT: usize = 1024;

#[tokio::test]
async fn mem_short_reply_says_early_eof() {
    // given: a raw initiator sends 10 bytes instead of the 32-byte one-time number and closes the write side
    let fx = setup(Opts::default()).await;
    // when: B responds
    let responded = fx.when_raw_initiator_sends_short_nonce(10).await;
    // then: Err, the text has "early eof" (it used to be "stream finished early")
    let text = format!(
        "{:#}",
        responded.expect_err("respond on a cut-off stream must refuse")
    );
    assert!(text.contains("early eof"), "error text: {text}");
}

#[tokio::test]
async fn mem_tail_over_limit_gives_no_name() {
    // given: a raw initiator with the right tag and a 1025-byte tail whose start is a valid name
    let fx = setup(Opts::default()).await;
    // when: B responds
    let responded = fx.when_raw_initiator_sends_tail(TAIL_LIMIT + 1).await;
    // then: Ok (an extra tail is not an error), A is a member, but the name is not recorded (the tail is dropped whole)
    assert!(responded.is_ok(), "respond: {responded:?}");
    let a = fx.ini.ctx.own_id;
    assert!(fx.res.ctx.members.is_member(a), "B did not add A");
    assert_eq!(
        fx.res.ctx.members.name_of(a),
        None,
        "name from a tail over the limit"
    );
}

#[tokio::test]
async fn mem_tail_at_limit_keeps_name() {
    // given: the same, but the tail is exactly 1024 bytes
    let fx = setup(Opts::default()).await;
    // when: B responds
    let responded = fx.when_raw_initiator_sends_tail(TAIL_LIMIT).await;
    // then: Ok and the name from the start of the tail is recorded
    assert!(responded.is_ok(), "respond: {responded:?}");
    let name = fx.res.ctx.members.name_of(fx.ini.ctx.own_id);
    assert_eq!(name, Some(NAME_A.to_string()), "name from a tail at the limit");
}

#[tokio::test]
async fn mem_silent_initiator_times_out_408() {
    // The project's `tokio` has no `test-util` (no `time::pause`, so no `start_paused`) — the test runs in real time, ~10 s; it fits in nextest (slow 15 s).
    // given: A opened a connection and is silent, no stream
    let fx = setup(Opts::default()).await;
    // when: B responds and waits for the handshake for 10 s
    let responded = fx.when_responding_alone(Duration::from_secs(20)).await;
    // then: Err, B closed the connection with code 408 and the reason timeout
    assert!(
        responded.is_err(),
        "respond to a silent initiator must refuse"
    );
    let expected = Some((408, Bytes::from_static(b"timeout")));
    assert_eq!(
        close_info(&fx.res.conn),
        expected,
        "close code on the responder"
    );
}

#[tokio::test]
async fn mem_responder_without_name_tail_is_accepted() {
    // given: a raw responder (an old node) answers `1` with no name frame
    let fx = setup(Opts::default()).await;
    // when: A (a real initiate) dials it
    let (initiated, _) = tokio::join!(
        must_finish(initiate(&fx.ini.conn, &fx.ini.ctx)),
        must_finish(fx.raw_responder_without_name())
    );
    // then: the handshake succeeds, B is a member, B has no name
    assert!(initiated.is_ok(), "initiate: {initiated:?}");
    let b = fx.res.ctx.own_id;
    assert!(fx.ini.ctx.members.is_member(b), "A did not add B");
    assert_eq!(fx.ini.ctx.members.name_of(b), None, "name from an empty tail");
}

// ---- helpers ----

impl Fixture {
    /// A raw responder goes through the protocol with the right tags, but sends only `1` at the end.
    async fn raw_responder_without_name(&self) {
        let StreamPair { mut send, mut recv } = self.res.conn.accept().await.expect("accept");
        let mut nonce_a = [0u8; 32];
        recv.read_exact(&mut nonce_a).await.expect("nonce_a");
        let nonce_b = [5u8; 32];
        let (me, peer) = (self.res.ctx.own_id, self.ini.ctx.own_id);
        let tag = responder_tag(&KEY, &nonce_a, &nonce_b, &peer, &me);
        send.write_all(&nonce_b).await.expect("nonce_b");
        send.write_all(tag.as_bytes()).await.expect("tag");
        let mut rest = Vec::new();
        recv.read_to_end(&mut rest).await.expect("the initiator's tag and tail");
        send.write_all(&[1u8]).await.expect("ok");
        send.shutdown().await.expect("shutdown");
    }

    /// A raw initiator cuts the write short after `len` bytes of the one-time number.
    async fn when_raw_initiator_sends_short_nonce(&self, len: usize) -> anyhow::Result<()> {
        let raw = async {
            let StreamPair {
                mut send,
                recv: _recv,
            } = self.ini.conn.open().await.expect("open");
            send.write_all(&vec![4u8; len]).await.expect("write");
            send.shutdown().await.expect("shutdown");
        };
        let (_, responded) = tokio::join!(must_finish(raw), must_finish(respond(&self.res.conn, &self.res.ctx)));
        responded
    }

    /// A raw initiator goes through the protocol with the right tag and a tail of `tail_len` bytes.
    async fn when_raw_initiator_sends_tail(&self, tail_len: usize) -> anyhow::Result<()> {
        let raw = self.raw_initiator(tail_for(tail_len));
        let (_, responded) = tokio::join!(must_finish(raw), must_finish(respond(&self.res.conn, &self.res.ctx)));
        responded
    }

    async fn raw_initiator(&self, tail: Vec<u8>) {
        let StreamPair { mut send, mut recv } = self.ini.conn.open().await.expect("open");
        let nonce_a = [4u8; 32];
        send.write_all(&nonce_a).await.expect("write nonce_a");
        let mut reply = [0u8; 64];
        recv.read_exact(&mut reply).await.expect("reply");
        let nonce_b: [u8; 32] = reply[..32].try_into().unwrap();
        let (me, peer) = (self.ini.ctx.own_id, self.res.ctx.own_id);
        send.write_all(initiator_tag(&KEY, &nonce_a, &nonce_b, &me, &peer).as_bytes())
            .await
            .expect("write tag");
        send.write_all(&tail).await.expect("write tail");
        send.shutdown().await.expect("shutdown");
    }
}

/// Handshake tail of length `len`: the name frame of A, the rest zeros (past the limit the tail is dropped
/// whole, so the name at the start is a sign the tail was parsed after all).
fn tail_for(len: usize) -> Vec<u8> {
    let mut tail = name_frame(NAME_A);
    tail.resize(len, 0);
    tail
}
