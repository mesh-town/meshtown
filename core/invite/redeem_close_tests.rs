//! Tests of closing the transport in `redeem_with` (`redeem.rs`): on any exit — exactly one `close`.
//!
//! The client is a `Spy` over `mem` (counts `close` and `connect`), the server is the `Responder` stub from `invite::test_responder`.
//! Closing the temporary `mem` is `ephemeral_mem_is_closed_on_exit`.
//!
//! assumption: `redeem_with(t: Arc<dyn Transport>, head, peers: &[PeerAddr], opts, progress)` and
//! `ask(t: &dyn Transport, addr: &PeerAddr, s: &InviteSecret) -> Option<RedeemReply>` are private
//! functions of `redeem.rs`; `bind_ephemeral` is in `node::start`;
//! the node seed in `Responder::start` matches the key of the transport passed to it.

use super::super::{ask, redeem_with, RedeemError, RedeemOpts};
use crate::invite::wire::{encode_invite_request, RedeemCode, RedeemReply, REPLY_LEN};
use crate::invite::InviteHead;
use crate::members::network_key::NetworkKey;
use crate::transport::net::facade::test_support::{addr, pid, secret, within, Spy};
use crate::transport::net::mem::conn::MemConn;
use crate::transport::net::{MemNet, PeerAddr, INVITE};
use crate::transport::net_mainline::unix_now;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;

// The stub server and the fixtures live in `test_responder`; the sibling tests take them from here.
use crate::invite::test_responder::refusal_frame;
pub(super) use crate::invite::test_responder::{
    head, ignore, key_frame, opts, own_key, owner, Responder,
};

#[tokio::test]
async fn key_closes_once() {
    // given: server B answers with the key of its network
    let rig = setup(key_frame());
    // when: the client joins
    let got = rig.when_redeems(&rig.peers(), &opts()).await;
    // then: the key is received, the transport is closed once, exactly B was asked over INVITE
    assert_key(got, &own_key());
    rig.then_closed_once();
    assert_eq!(
        rig.spy.connects(),
        vec![(pid(2), INVITE)],
        "the wrong node or the wrong service was asked"
    );
}

#[tokio::test]
async fn final_refusal_closes_once() {
    // given: server B answers "the invite has already been used"
    let rig = setup(refusal_frame(RedeemCode::Used));
    // when
    let got = rig.when_redeems(&rig.peers(), &opts()).await;
    // then: a final refusal and one close
    assert!(matches!(got, Err(RedeemError::Used)), "{got:?}");
    rig.then_closed_once();
}

#[tokio::test]
async fn unknown_closes_once() {
    // given: server B answers "unknown"; one "unknown" round is the outcome; pauses of 30 s
    let rig = setup(refusal_frame(RedeemCode::Unknown));
    // when: the client joins (a pause between rounds would break the `within` deadline)
    let got = rig.when_redeems(&rig.peers(), &opts()).await;
    // then: Unknown without a pause, one close
    assert!(matches!(got, Err(RedeemError::Unknown)), "{got:?}");
    rig.then_closed_once();
}

#[tokio::test]
async fn expired_before_first_round_closes_once() {
    // given: the invite's deadline passed before the first round
    let rig = setup(key_frame());
    let expired = RedeemOpts {
        deadline_unix: unix_now().saturating_sub(10),
        ..opts()
    };
    // when
    let got = rig.when_redeems(&rig.peers(), &expired).await;
    // then: Expired, no connections, but the transport is closed all the same
    assert!(matches!(got, Err(RedeemError::Expired)), "{got:?}");
    assert!(
        rig.spy.connects().is_empty(),
        "a connection was opened after the deadline: {:?}",
        rig.spy.connects()
    );
    rig.then_closed_once();
}

#[tokio::test]
async fn unreachable_peer_closes_once() {
    // given: the address of a node that is not in the mem network; the deadline in a second, pauses of 20 ms
    let rig = setup(key_frame());
    let nowhere = [addr(9, &[])];
    let short = RedeemOpts {
        first_pause: ms(20),
        max_pause: ms(20),
        deadline_unix: unix_now() + 1,
        ..opts()
    };
    // when: rounds go on until the deadline passes
    let got = rig.when_redeems(&nowhere, &short).await;
    // then: Expired and one close
    assert!(matches!(got, Err(RedeemError::Expired)), "{got:?}");
    rig.then_closed_once();
}

#[tokio::test]
async fn asked_conn_is_closed() {
    // given: server B answers with the key; the client's transport stays open, so
    // only `ask` itself can close the connection on the server side
    let rig = setup(key_frame());
    // when: one exchange
    let reply = rig.when_asks_once().await;
    // then: the reply is received, the connection on the server side is closed by the client (code 0, reason "done")
    assert!(matches!(reply, Some(RedeemReply::Key(_))), "{reply:?}");
    rig.then_server_conn_closed_by_client().await;
}

#[tokio::test]
async fn ephemeral_mem_is_closed_on_exit() {
    // given: a temporary `mem` transport, the invite's deadline has already passed, no addresses
    let net = MemNet::new();
    let t = crate::node::bind_ephemeral(Some(&net))
        .await
        .expect("the temporary transport came up");
    let me = t.me();
    let expired = RedeemOpts {
        deadline_unix: unix_now().saturating_sub(10),
        ..opts()
    };
    // when
    let got = within(
        "redeem_with",
        redeem_with(t, &head(), &[], &expired, &ignore, &owner()),
    )
    .await;
    // then: Expired, the slot is closed by redeem_with itself
    assert!(matches!(got, Err(RedeemError::Expired)), "{got:?}");
    assert!(
        net.addr_of(me).is_none(),
        "the slot was not closed on the exit of redeem_with"
    );
}

/// The spy client A, the stub server B and the invite head of the network `own_key()`.
struct Rig {
    spy: Arc<Spy>,
    server: Responder,
    head: InviteHead,
    _net: Arc<MemNet>,
}

/// Server B answers any request with the frame `reply`.
fn setup(reply: [u8; REPLY_LEN]) -> Rig {
    let net = MemNet::new();
    let spy = Spy::on(&net, &secret(1)).shared();
    let server = Responder::start(net.transport(&secret(2)), 2, reply);
    Rig {
        spy,
        server,
        head: head(),
        _net: net,
    }
}

impl Rig {
    /// The address of server B.
    fn peers(&self) -> Vec<PeerAddr> {
        vec![self.server.addr()]
    }

    async fn when_redeems(
        &self,
        peers: &[PeerAddr],
        opts: &RedeemOpts,
    ) -> Result<NetworkKey, RedeemError> {
        let owner = owner();
        let done = redeem_with(self.spy.clone(), &self.head, peers, opts, &ignore, &owner);
        within("redeem_with", done).await
    }

    /// One call of `ask` without closing the transport.
    async fn when_asks_once(&self) -> Option<RedeemReply> {
        let addr = self.server.addr();
        let frame = encode_invite_request(&self.head.secret);
        let asked = ask(&*self.spy, &addr, &frame);
        within("ask", asked).await
    }

    fn then_closed_once(&self) {
        assert_eq!(
            self.spy.closes(),
            1,
            "the client's transport was not closed exactly once"
        );
    }

    async fn then_server_conn_closed_by_client(&self) {
        let conns = self.server.conns();
        assert_eq!(
            conns.len(),
            1,
            "the server must have exactly one connection"
        );
        let at_server = &conns[0];
        within("closed() on the server side", at_server.closed()).await;
        assert!(
            at_server.is_closed(),
            "the connection on the server side is not closed after the exchange"
        );
        // assumption: the server on mem is `MemConn`; `close_info()` is its test handle
        let info = at_server
            .as_any()
            .downcast_ref::<MemConn>()
            .and_then(MemConn::close_info);
        assert_eq!(
            info,
            Some((0, Bytes::from_static(b"done"))),
            "the close code and reason are not the previous ones"
        );
    }
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn assert_key(got: Result<NetworkKey, RedeemError>, want: &NetworkKey) {
    match got {
        Ok(key) => assert_eq!(
            key.to_bytes(),
            want.to_bytes(),
            "got a key of the wrong network"
        ),
        Err(e) => panic!("expected a key, but joining ended with an error: {e:?}"),
    }
}
