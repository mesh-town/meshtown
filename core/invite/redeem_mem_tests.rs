//! Joining by an invite over the temporary `mem` transport (`node::bind_ephemeral`): dialing by key,
//! a vanished peer, closing the slot on exit. The server is the `Responder` stub from `closes_transport`.

use super::super::{redeem, RedeemError};
use super::closes_transport::{head, ignore, key_frame, opts, own_key, owner, Responder};
use crate::invite::server::InviteServer;
use crate::invite::store::{issue_record, Ledger};
use crate::invite::wire::{encode_invite_reply, RedeemCode, REQUEST_LEN};
use crate::members::access::record::admit_key;
use crate::members::access::tally::Ledger as Votes;
use crate::members::access::view::{AccessView, Verdicts};
use crate::proxy::domains::DomainList;
use crate::transport::net::facade::test_support::{pid, secret, within};
use crate::transport::net::mem::MemSyncNet;
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{
    BoxFuture, NetStackBuilder, PeerConn, Replica, ServiceHandler, StreamPair, INVITE,
};
use crate::transport::net::{MemNet, NetStack, PeerAddr};
use std::io;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn redeem_by_key_ok() {
    let net = MemNet::new();
    let _server = Responder::start(net.transport(&secret(2)), 2, key_frame());
    let t = crate::node::bind_ephemeral(Some(&net)).await.unwrap();
    let peers = [PeerAddr::new(pid(2))];
    let got = within(
        "redeem",
        redeem(&head(), &peers, &opts(), &ignore, t, &owner()),
    )
    .await;
    assert_eq!(
        got.unwrap().subkeys().network_id,
        own_key().subkeys().network_id
    );
}

#[tokio::test]
async fn redeem_peer_gone() {
    let net = MemNet::new();
    let t = crate::node::bind_ephemeral(Some(&net)).await.unwrap();
    let mut o = opts();
    o.deadline_unix = crate::transport::net_mainline::unix_now().saturating_sub(10);
    let peers = [PeerAddr::new(pid(2))];
    let got = within("redeem", redeem(&head(), &peers, &o, &ignore, t, &owner())).await;
    assert!(matches!(got, Err(RedeemError::Expired)), "{got:?}");
}

#[tokio::test]
async fn redeem_closes_transport() {
    let net = MemNet::new();
    let _server = Responder::start(net.transport(&secret(2)), 2, key_frame());
    let t = crate::node::bind_ephemeral(Some(&net)).await.unwrap();
    let me = t.me();
    assert!(
        net.addr_of(me).is_some(),
        "the slot is alive before joining"
    );
    let peers = [PeerAddr::new(pid(2))];
    within(
        "redeem",
        redeem(&head(), &peers, &opts(), &ignore, t, &owner()),
    )
    .await
    .unwrap();
    assert!(net.addr_of(me).is_none(), "the slot was not closed on exit");
}

mod real_server {
    use super::*;

    #[tokio::test]
    async fn real_server_admits_owner_key() {
        // given: a member holding the invite "Masha" on the mem network, and a joining node with its own key
        let net = MemNet::new();
        let member = member_with_the_invite(&net).await;
        let t = crate::node::bind_ephemeral(Some(&net)).await.unwrap();
        let peers = [PeerAddr::new(pid(2))];
        // when: the node joins by the invite with the owner frame
        let got = within(
            "redeem",
            redeem(&head(), &peers, &opts(), &ignore, t, &owner()),
        )
        .await;
        // then: the network key arrives, and the member has admitted the owner by its permanent key first
        assert_eq!(
            got.unwrap().subkeys().network_id,
            own_key().subkeys().network_id
        );
        let admit = member
            .shared
            .get(pid(2), &admit_key(&owner().peer_id()))
            .await
            .unwrap();
        assert!(
            admit.is_some(),
            "the member did not write the admit record of the owner"
        );
    }
}

mod old_server {
    use super::*;

    #[tokio::test]
    async fn old_server_stub_gets_legacy_frame_after_fallback() {
        // given: a member of an older version: it answers BadRequest to everything but the kind 1 frame
        let net = MemNet::new();
        let kinds = Arc::new(Mutex::new(Vec::new()));
        let _stack = old_member(&net, kinds.clone());
        let t = crate::node::bind_ephemeral(Some(&net)).await.unwrap();
        let peers = [PeerAddr::new(pid(2))];
        // when: the node joins by the invite
        let got = within(
            "redeem",
            redeem(&head(), &peers, &opts(), &ignore, t, &owner()),
        )
        .await;
        // then: the key arrives, and the member saw the owner frame (3) first, then the legacy frame (1)
        assert_eq!(
            got.unwrap().subkeys().network_id,
            own_key().subkeys().network_id
        );
        let seen = kinds.lock().unwrap_or_else(PoisonError::into_inner).clone();
        assert_eq!(seen, vec![3, 1], "kinds of the frames the old member saw");
    }
}

/// The view of the member `pid(2)`, the administrator of its network.
fn admin_view() -> AccessView {
    let mut ledger = Votes::empty();
    ledger.admitted.insert(pid(2));
    ledger.admins.insert(pid(2));
    let access = AccessView::open();
    access.publish(Verdicts {
        me: pid(2),
        ledger: Arc::new(ledger),
        blocks: Default::default(),
    });
    access
}

/// A member node with a real invite server over a filled in-memory list.
struct Member {
    shared: Arc<dyn Replica>,
    _keep: (NetStack, Arc<MemSyncNet>),
}

async fn member_with_the_invite(net: &Arc<MemNet>) -> Member {
    let sync = MemSyncNet::new();
    let backend = sync.backend(SyncCtx {
        secret: secret(2),
        network_seed: [9; 32],
        store_dir: None,
    });
    let shared: Arc<dyn Replica> = backend.replica_mem(REPLICA_NET).unwrap();
    let own: Arc<dyn Replica> = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let list = DomainList::open(shared.clone(), own).await.unwrap();
    let record = issue_record(crate::transport::net_mainline::unix_now(), Some("Masha"));
    Ledger::of(&list)
        .issue_as(&head().secret.hash_hex(), &record, Some(pid(2)))
        .await
        .unwrap();
    let cell = Arc::new(OnceLock::new());
    drop(cell.set(list));
    let server = InviteServer::new(own_key(), cell, admin_view(), own_key());
    let stack = NetStackBuilder::new(secret(2))
        .transport(net.transport(&secret(2)))
        .serve(INVITE, Arc::new(server))
        .build();
    Member {
        shared,
        _keep: (stack.expect("the member stack was built"), sync),
    }
}

/// A member that does not know the owner frame: BadRequest to kind 3, the network key to kind 1.
fn old_member(net: &Arc<MemNet>, kinds: Arc<Mutex<Vec<u8>>>) -> NetStack {
    let handler = Arc::new(OldMember { kinds });
    let built = NetStackBuilder::new(secret(2))
        .transport(net.transport(&secret(2)))
        .serve(INVITE, handler)
        .build();
    built.expect("the stub stack was built")
}

struct OldMember {
    kinds: Arc<Mutex<Vec<u8>>>,
}

impl ServiceHandler for OldMember {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let kinds = self.kinds.clone();
        Box::pin(async move {
            let _ = answer_as_old(conn.as_ref(), kinds).await;
            conn.closed().await;
        })
    }
}

async fn answer_as_old(conn: &dyn PeerConn, kinds: Arc<Mutex<Vec<u8>>>) -> io::Result<()> {
    let accepted = conn.accept().await;
    let StreamPair { mut send, mut recv } =
        accepted.map_err(|e| io::Error::other(e.to_string()))?;
    let mut request = [0u8; REQUEST_LEN];
    recv.read_exact(&mut request).await?;
    kinds
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request[0]);
    let reply = match request[0] {
        1 => key_frame(),
        _ => encode_invite_reply(RedeemCode::BadRequest, None),
    };
    send.write_all(&reply).await?;
    send.shutdown().await
}
