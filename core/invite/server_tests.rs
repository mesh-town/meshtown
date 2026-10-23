//! Tests of the invite server (`server.rs`) on the in-memory network `mem`: the reply code for a request frame,
//! busy slots, returning a slot after the answer, waiting for the client to close, a cut-off request.
//!
//! The server here is a real `InviteServer` with an empty domain list: verdicts by the document
//! (`Ok`, `Used`, `Expired`, `Unknown`) are checked by `core/tests/invite_once_loopback.rs`, here they
//! are not needed. The client is a second `mem` node that assembles frames itself; the handler
//! `ServiceHandler::handle` is run in the test's task, to see when it finished.
//!
//! assumption: `InviteServer::new(key, list, access)`, a private field `slots: Arc<Semaphore>` with
//! `MAX_REDEEMS` slots, `ServiceHandler::handle(&self, conn) -> BoxFuture<'static, ()>`; the handler
//! reads the request from the stream, releases the slot right after the answer and waits for `conn.closed()` (≤ 5 s).

use super::{InviteServer, ANSWER_TIMEOUT, CLAIM_SETTLE, MAX_REDEEMS};
use crate::invite::hold_wire::{hold_frame, parse_hold_reply, HOLD_LEN};
use crate::invite::proof::sign_owner;
use crate::invite::redeem::EXCHANGE_LIMIT;
use crate::invite::wire::{
    encode_invite_request, parse_invite_reply, RedeemCode, RedeemReply, REDEEM_FRAME_KIND,
    REPLY_LEN, REQUEST_LEN,
};
use crate::invite::wire::{encode_owner_request, OWNER_REQUEST_LEN};
use crate::invite::InviteSecret;
use crate::members::access::view::AccessView;
use crate::members::network_key::NetworkKey;
use crate::proxy::domains::DomainList;
use crate::transport::net::facade::test_support::{pid, secret, within};
use crate::transport::net::{
    MemNet, NodeSecret, PeerConn, PeerId, ServiceHandler, StreamPair, Transport, INVITE,
};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::OwnedSemaphorePermit;
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// How long the handler must hold on after the answer, until the client closes the connection.
const HOLD: Duration = Duration::from_millis(300);

mod answer_to_the_request_frame {
    use super::*;

    #[tokio::test]
    async fn broken_frame_gets_bad_request() {
        // given: a free server and a request frame with a broken header
        let fx = setup(Opts::default());
        let session = fx.when_client_connects().await;
        // when: the client sends this frame
        let reply = session.when_asks(&broken_frame()).await;
        // then: the BadRequest refusal, no key in the reply
        assert_refused(reply, RedeemCode::BadRequest);
    }

    #[tokio::test]
    async fn owner_frame_before_list_gets_not_ready() {
        // given: a valid owner frame, the domain list is not filled yet (the OnceLock is empty)
        let fx = setup(Opts::default());
        let session = fx.when_client_connects().await;
        // when
        let reply = session
            .when_asks(&owner_frame(
                &secret(7),
                &InviteSecret::from_bytes([7; 32]),
                pid(1),
            ))
            .await;
        // then
        assert_refused(reply, RedeemCode::NotReady);
    }

    #[tokio::test]
    async fn full_slots_get_busy() {
        // given: all MAX_REDEEMS server slots are taken
        let fx = setup(Opts {
            slots_full: true,
            ..Opts::default()
        });
        let session = fx.when_client_connects().await;
        // when: one more valid request arrives
        let reply = session.when_asks(&valid_frame()).await;
        // then: Busy (the request was read, otherwise there would be no reply)
        assert_refused(reply, RedeemCode::Busy);
    }
}

mod slot_of_the_request {
    use super::*;

    #[tokio::test]
    async fn slot_is_back_after_the_answer() {
        // given: a free server and a client that keeps the connection open
        let fx = setup(Opts::default());
        let session = fx.when_client_connects().await;
        // when: the reply is read in full, the client did not close the connection
        session.when_asks(&valid_frame()).await;
        // then: the slot is already released (after answer, before sending and waiting for the close)
        assert_eq!(
            fx.slots_free(),
            MAX_REDEEMS,
            "the slot did not come back after the answer, until the client closed the connection"
        );
    }
}

mod handler_lifetime {
    use super::*;

    #[tokio::test]
    async fn handler_waits_for_client_close() {
        // given: the reply is read in full
        let fx = setup(Opts::default());
        let mut session = fx.when_client_connects().await;
        session.when_asks(&valid_frame()).await;
        // when: 300 ms pass, the client does not close the connection
        sleep(HOLD).await;
        // then: the handler is still alive (otherwise the connection would drop and the reply would be lost)
        assert!(
            session.then_handler_runs(),
            "handle returned before the client closed"
        );
        // when: the client closes the connection with code 0 and the reason "done"
        session.when_client_closes();
        // then: the handler finishes within STEP
        session.then_handler_ends().await;
    }

    #[tokio::test]
    async fn short_request_ends_the_handler() {
        // given: a client that will send fewer than REQUEST_LEN bytes
        let fx = setup(Opts::default());
        let mut session = fx.when_client_connects().await;
        // when: half a frame and the end of its half of the stream
        let reply = session.when_asks(&valid_frame()[..REQUEST_LEN / 2]).await;
        // then: no reply, the handler finished within STEP
        assert!(
            reply.is_none(),
            "a reply came to a cut-off request: {reply:?}"
        );
        session.then_handler_ends().await;
    }
}

mod hold_frame_answers {
    use super::*;

    #[tokio::test]
    async fn hold_before_list_gets_not_ready_v2() {
        // given: a correctly signed fresh HOLD, the domain list is not filled yet
        let fx = setup(Opts::default());
        let session = fx.when_client_connects().await;
        // when
        let raw = session.when_asks_raw(&signed_hold(KEY)).await;
        // then: a kind 2 reply with the code NotReady
        assert_eq!(
            raw.and_then(|b| parse_hold_reply(&b)),
            Some(RedeemCode::NotReady)
        );
    }

    #[tokio::test]
    async fn hold_with_wrong_mac_gets_bad_request_v2() {
        // given: a HOLD signed with a foreign key (the list is not needed: the signature is checked first)
        let fx = setup(Opts::default());
        let session = fx.when_client_connects().await;
        // when
        let raw = session.when_asks_raw(&signed_hold([9; 32])).await;
        // then
        assert_eq!(
            raw.and_then(|b| parse_hold_reply(&b)),
            Some(RedeemCode::BadRequest)
        );
    }

    #[tokio::test]
    async fn short_hold_frame_ends_the_handler() {
        // given: 40 bytes of a HOLD frame and the end of the stream
        let fx = setup(Opts::default());
        let mut session = fx.when_client_connects().await;
        // when
        let raw = session.when_asks_raw(&signed_hold(KEY)[..40]).await;
        // then: no reply, the handler finished
        assert!(raw.is_none(), "a reply came to a cut-off HOLD: {raw:?}");
        session.then_handler_ends().await;
    }

    #[test]
    fn claim_settle_fits_the_old_client_limit() {
        // the old client waits EXCHANGE_LIMIT; the wait plus two mark writes must fit into it
        let two_writes = Duration::from_secs(2);
        assert!(CLAIM_SETTLE + two_writes <= EXCHANGE_LIMIT);
        assert!(CLAIM_SETTLE + two_writes <= ANSWER_TIMEOUT);
    }
}

/// The network key with which `setup` builds the server.
const KEY: [u8; 32] = [1; 32];

/// A fresh HOLD signed with the key `key`.
fn signed_hold(key: [u8; 32]) -> [u8; HOLD_LEN] {
    let now = crate::transport::net_mainline::unix_now();
    hold_frame(&NetworkKey::from_bytes(key), &[5; 32], now)
}

/// The server's preconditions.
#[derive(Default)]
struct Opts {
    /// All `MAX_REDEEMS` slots are taken before the first request.
    slots_full: bool,
    /// What the server asks about the newcomer's key; `open` (lets everyone in) by default.
    access: AccessView,
    /// The filled domain list; none by default (the cell stays empty).
    list: Option<DomainList>,
}

/// The server on node B of the `mem` network and the client on node A.
struct Fixture {
    server: InviteServer,
    client: Arc<dyn Transport>,
    inbox: Arc<dyn Transport>,
    _held: Vec<OwnedSemaphorePermit>,
    _net: Arc<MemNet>,
}

fn setup(opts: Opts) -> Fixture {
    let net = MemNet::new();
    let client: Arc<dyn Transport> = net.transport(&secret(1));
    let inbox: Arc<dyn Transport> = net.transport(&secret(2));
    inbox.serve(&[INVITE]);
    let cell = Arc::new(OnceLock::new());
    opts.list.into_iter().for_each(|list| drop(cell.set(list)));
    let server = InviteServer::new(NetworkKey::from_bytes(KEY), cell, opts.access);
    let held = match opts.slots_full {
        true => take_all_slots(&server),
        false => Vec::new(),
    };
    Fixture {
        server,
        client,
        inbox,
        _held: held,
        _net: net,
    }
}

fn take_all_slots(server: &InviteServer) -> Vec<OwnedSemaphorePermit> {
    let take = |_| {
        server
            .slots
            .clone()
            .try_acquire_owned()
            .expect("the slot is free")
    };
    (0..MAX_REDEEMS).map(take).collect()
}

impl Fixture {
    /// The client connects over `INVITE`, B accepts and gives the connection to the server's handler.
    async fn when_client_connects(&self) -> Session {
        let to = self.inbox.local_addr();
        let conn = within("connect", self.client.connect(&to, INVITE)).await;
        let conn = conn.expect("connect to the invite server succeeded");
        let (svc, at_server) = within("accept", self.inbox.accept())
            .await
            .expect("B accepted the connection");
        assert_eq!(svc, INVITE, "the connection came not to the invite service");
        let handler = crate::node::output::spawn(self.server.handle(at_server));
        Session { conn, handler }
    }

    fn slots_free(&self) -> usize {
        self.server.slots.available_permits()
    }
}

/// The client's connection and the handler task on the server side.
struct Session {
    conn: Arc<dyn PeerConn>,
    handler: JoinHandle<()>,
}

impl Session {
    /// The client sends `bytes`, closes its half of the stream and reads the reply; `None` — no reply.
    async fn when_asks(&self, bytes: &[u8]) -> Option<RedeemReply> {
        let opened = within("open", self.conn.open()).await;
        let StreamPair { mut send, mut recv } = opened.expect("the stream opened");
        send.write_all(bytes)
            .await
            .expect("the request was written");
        send.shutdown().await.expect("the half was closed");
        let mut buf = [0u8; REPLY_LEN];
        within("reading the reply", recv.read_exact(&mut buf))
            .await
            .ok()?;
        parse_invite_reply(&buf)
    }

    /// The same, but the reply is the raw 34 bytes without parsing; `None` — no reply.
    async fn when_asks_raw(&self, bytes: &[u8]) -> Option<[u8; REPLY_LEN]> {
        let opened = within("open", self.conn.open()).await;
        let StreamPair { mut send, mut recv } = opened.expect("the stream opened");
        send.write_all(bytes)
            .await
            .expect("the request was written");
        send.shutdown().await.expect("the half was closed");
        let mut buf = [0u8; REPLY_LEN];
        within("reading the reply", recv.read_exact(&mut buf))
            .await
            .ok()?;
        Some(buf)
    }

    fn when_client_closes(&self) {
        self.conn.close(0, b"done");
    }

    fn then_handler_runs(&self) -> bool {
        !self.handler.is_finished()
    }

    async fn then_handler_ends(&mut self) {
        let ended = within("handle finishing", &mut self.handler).await;
        assert!(ended.is_ok(), "the handler task crashed: {ended:?}");
    }
}

/// A valid request frame.
fn valid_frame() -> [u8; REQUEST_LEN] {
    encode_invite_request(&InviteSecret::from_bytes([7; 32]))
}

/// A frame of the right length with a foreign version in the header.
fn broken_frame() -> [u8; REQUEST_LEN] {
    let mut frame = valid_frame();
    frame[0] = REDEEM_FRAME_KIND.wrapping_add(3); // +1 is the HOLD kind, +2 is the owner kind
    frame
}

/// An owner frame for `s` signed by `owner` for the connection whose client transport is `transport`.
fn owner_frame(owner: &NodeSecret, s: &InviteSecret, transport: PeerId) -> [u8; OWNER_REQUEST_LEN] {
    encode_owner_request(s, owner.peer_id(), &sign_owner(owner, s, transport))
}

fn assert_refused(reply: Option<RedeemReply>, want: RedeemCode) {
    match reply {
        Some(RedeemReply::Refused(got)) => {
            assert_eq!(got, want, "the refusal code is not the expected one")
        }
        other => panic!("expected the refusal {want:?}, got {other:?}"),
    }
}

#[path = "server_owner_frame_tests.rs"]
mod owner_frame_flow;
