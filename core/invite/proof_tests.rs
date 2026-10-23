//! Tests of the owner proof and of the admit record the server writes for an invited node (`proof.rs`).
//!
//! The proof signs `CTX ‖ hash of the invite secret ‖ transport id ‖ owner id`: the transport id ties the
//! signature to one connection, so a frame seen by a third party cannot be replayed from another transport.
//! The admit write goes to an in-memory replica; the full server path is in `server_owner_frame_tests.rs`.

use std::sync::Arc;

use bytes::Bytes;
use data_encoding::HEXLOWER;

use super::{admit_owner, issue_note, owner_message, owner_proven, sign_owner, CTX_INVITE_OWNER};
use crate::invite::store::{issue_record, IssueRecord, Ledger};
use crate::invite::wire::OwnerRequest;
use crate::invite::InviteSecret;
use crate::members::access::record::{admit_key, parse_entry, AclKind, AclValue, Parsed, Via};
use crate::proxy::domains::DomainList;
use crate::transport::net::contract::{pid, secret};
use crate::transport::net::mem::{MemSync, MemSyncNet};
use crate::transport::net::sync_backend::{SyncCtx, REPLICA_DOMAINS, REPLICA_NET};
use crate::transport::net::{NodeSecret, PeerId, Replica, ReplicaEntry};

const NOW: u64 = 1_792_400_000;

/// Two public keys of RFC 8032 test vectors: real curve points with fixed bytes.
const TRANSPORT_HEX: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
const OWNER_HEX: &str = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c";
/// `CTX ‖ [0x11; 32] ‖ TRANSPORT ‖ OWNER`, written out byte by byte.
const GOLDEN_MESSAGE_HEX: &str = concat!(
    "6d657368746f776e20696e76697465206f776e65722070726f6f66207631",
    "00",
    "1111111111111111111111111111111111111111111111111111111111111111",
    "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
    "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
);

mod message {
    use super::*;

    #[test]
    fn owner_message_golden_bytes() {
        // given: a fixed hash, a fixed transport id and a fixed owner id
        let (transport, owner) = (peer(TRANSPORT_HEX), peer(OWNER_HEX));
        // when
        let bytes = owner_message(&[0x11; 32], transport, owner);
        // then: exactly the golden bytes (catches a changed separator, order or context)
        assert_eq!(HEXLOWER.encode(&bytes), GOLDEN_MESSAGE_HEX);
        assert_eq!(
            CTX_INVITE_OWNER,
            b"meshtown invite owner proof v1\0".as_slice()
        );
    }

    #[test]
    fn signature_covers_the_hash_of_the_secret_not_the_secret() {
        // given: an owner key and an invite secret
        let (owner, s, transport) = (secret(7), invite(1), pid(1));
        // when
        let sig = sign_owner(&owner, &s, transport);
        // then: it verifies over the message built from the hash, and not over one built from the raw secret
        let by_hash = owner_message(&s.hash_bytes(), transport, owner.peer_id());
        let by_secret = owner_message(s.as_bytes(), transport, owner.peer_id());
        assert!(
            owner.peer_id().verify(&by_hash, &sig),
            "the signature is not over the hash"
        );
        assert!(
            !owner.peer_id().verify(&by_secret, &sig),
            "the raw secret was signed"
        );
    }
}

mod proof {
    use super::*;

    #[test]
    fn proof_verifies_for_same_transport() {
        // given: a request signed for the transport pid(1)
        let req = request(&secret(7), &invite(1), pid(1));
        // when / then
        assert!(owner_proven(&req, pid(1)));
    }

    #[test]
    fn proof_fails_for_other_transport() {
        // given: the same request seen on another connection
        let req = request(&secret(7), &invite(1), pid(1));
        // when / then: a replay from another transport is not accepted
        assert!(
            !owner_proven(&req, pid(2)),
            "a proof for one transport held on another"
        );
    }

    #[test]
    fn proof_fails_for_other_secret() {
        // given: a signature made for one invite secret, put into a request with another
        let mut req = request(&secret(7), &invite(1), pid(1));
        req.secret = invite(2);
        // when / then
        assert!(
            !owner_proven(&req, pid(1)),
            "a proof for one invite held for another"
        );
    }

    #[test]
    fn proof_signed_by_other_key_fails() {
        // given: the owner field names key 7, the signature is made by key 8
        let mut req = request(&secret(8), &invite(1), pid(1));
        req.owner = secret(7).peer_id();
        // when / then
        assert!(
            !owner_proven(&req, pid(1)),
            "a signature of a foreign key proved the ownership"
        );
    }

    #[test]
    fn proof_with_a_damaged_signature_fails() {
        // given: a good proof with one flipped bit
        let mut req = request(&secret(7), &invite(1), pid(1));
        req.sig[10] ^= 1;
        // when / then
        assert!(!owner_proven(&req, pid(1)));
    }
}

mod issue_note_of_records {
    use super::*;

    #[test]
    fn note_taken_from_record_with_for_whom() {
        // given: the issuer's record "Masha" and a newer HOLD copy of a holder without a name
        let entries = [
            issue_entry(pid(2), NOW, None),
            issue_entry(pid(3), NOW - 50, Some("Masha")),
        ];
        // when / then: the name comes from the record that has one
        assert_eq!(issue_note(&entries), "Masha");
    }

    #[test]
    fn newest_record_with_for_whom_wins() {
        // given: two records with names, the older one is first
        let entries = [
            issue_entry(pid(2), NOW - 100, Some("Old")),
            issue_entry(pid(3), NOW, Some("New")),
        ];
        // when / then
        assert_eq!(issue_note(&entries), "New");
    }

    #[test]
    fn note_empty_without_for_whom() {
        // given: only copies of holders and nothing at all
        let copies = [
            issue_entry(pid(2), NOW, None),
            issue_entry(pid(3), NOW, None),
        ];
        // when / then
        assert_eq!(issue_note(&copies), "");
        assert_eq!(issue_note(&[]), "");
    }

    #[test]
    fn note_cut_to_64_chars() {
        // given: records written by a node that did not cut the name (the cut is on our side too)
        for ch in ["a", "é"] {
            let name = ch.repeat(100);
            let entries = [issue_entry(pid(2), NOW, Some(name.as_str()))];
            // when
            let note = issue_note(&entries);
            // then: 64 characters, whatever their width
            assert_eq!(note.chars().count(), 64, "{ch}");
            assert_eq!(note, ch.repeat(64));
        }
    }

    #[test]
    fn tombstone_and_garbage_are_skipped() {
        // given: a newer tombstone, newer garbage bytes and one good record
        let tomb = ReplicaEntry {
            value: Bytes::new(),
            ..issue_entry(pid(2), NOW + 10, Some("Gone"))
        };
        let garbage = ReplicaEntry {
            value: Bytes::from_static(&[0xFF, 0xFF]),
            ..issue_entry(pid(3), NOW + 20, None)
        };
        let entries = [tomb, garbage, issue_entry(pid(4), NOW - 5, Some("Masha"))];
        // when / then: only the good record counts
        assert_eq!(issue_note(&entries), "Masha");
    }
}

mod admit_write {
    use super::*;

    #[tokio::test]
    async fn admit_owner_writes_invite_admit_with_the_issue_note() {
        // given: a node that issued the invite "Masha"
        let node = setup().await;
        node.when_issued(&invite(1), Some("Masha")).await;
        // when: the owner is admitted for this invite
        admit_owner(&node.list, &invite(1).hash_hex(), pid(7), None, NOW)
            .await
            .unwrap();
        // then: the node's own record admits the owner by invite, with the name as the note
        let want = AclValue {
            kind: AclKind::Admit,
            subject: pid(7),
            via: Via::Invite,
            made_unix: NOW,
            note: "Masha".into(),
        };
        assert_eq!(node.admit_of(pid(7)).await, Some(want));
    }

    #[tokio::test]
    async fn admit_owner_without_an_issue_record_leaves_the_note_empty() {
        // given: a node that has no record for this invite
        let node = setup().await;
        // when
        admit_owner(&node.list, &invite(1).hash_hex(), pid(7), None, NOW)
            .await
            .unwrap();
        // then
        let admit = node.admit_of(pid(7)).await.expect("the admit was written");
        assert_eq!((admit.via, admit.note.as_str()), (Via::Invite, ""));
    }

    #[tokio::test]
    async fn holder_redeem_writes_an_invite_vote() {
        // given: only the copy of the holder, which carries no label
        let node = setup().await;
        node.when_issued(&invite(1), None).await;
        // when: the holder admits the owner
        admit_owner(&node.list, &invite(1).hash_hex(), pid(7), None, NOW)
            .await
            .unwrap();
        // then: the record is an invite vote with an empty note
        let admit = node.admit_of(pid(7)).await.expect("the admit was written");
        assert_eq!((admit.via, admit.note.as_str()), (Via::Invite, ""));
    }

    #[tokio::test]
    async fn admit_owner_twice_keeps_one_record() {
        // given: an owner already admitted (a retry after a failed mark)
        let node = setup().await;
        admit_owner(&node.list, &invite(1).hash_hex(), pid(7), None, NOW)
            .await
            .unwrap();
        // when: the same admit is written again, later
        let again = admit_owner(&node.list, &invite(1).hash_hex(), pid(7), None, NOW + 3).await;
        // then: no error and still one live record under the key
        assert!(again.is_ok(), "{again:?}");
        assert_eq!(node.live_admits().await, 1);
    }
}

fn peer(hex: &str) -> PeerId {
    let bytes: [u8; 32] = HEXLOWER.decode(hex.as_bytes()).unwrap().try_into().unwrap();
    PeerId::from_bytes(&bytes).expect("a point of the curve")
}

fn invite(seed: u8) -> InviteSecret {
    InviteSecret::from_bytes([seed; 32])
}

/// A request signed by `signer` for `transport`, naming the signer as the owner.
fn request(signer: &NodeSecret, s: &InviteSecret, transport: PeerId) -> OwnerRequest {
    OwnerRequest {
        secret: s.clone(),
        owner: signer.peer_id(),
        sig: sign_owner(signer, s, transport),
    }
}

/// A live `invite/<hash>` entry by `author`; `ts_micros` follows `made_unix`.
fn issue_entry(author: PeerId, made_unix: u64, for_whom: Option<&str>) -> ReplicaEntry {
    let rec = IssueRecord {
        v: 1,
        made_unix,
        for_whom: for_whom.map(str::to_string),
    };
    ReplicaEntry {
        author,
        key: format!("invite/{}", invite(1).hash_hex()).into_bytes(),
        value: Bytes::from(postcard::to_stdvec(&rec).unwrap()),
        ts_micros: made_unix * 1_000_000,
    }
}

/// A node with its in-memory shared replica and a domain list over it.
struct Node {
    list: DomainList,
    me: PeerId,
    shared: Arc<dyn Replica>,
    _keep: (Arc<MemSyncNet>, Arc<MemSync>),
}

async fn setup() -> Node {
    let net = MemSyncNet::new();
    let me = secret(3);
    let backend = net.backend(SyncCtx {
        secret: me.clone(),
        network_seed: [9; 32],
        store_dir: None,
    });
    let shared: Arc<dyn Replica> = backend.replica_mem(REPLICA_NET).unwrap();
    let own: Arc<dyn Replica> = backend.replica_mem(REPLICA_DOMAINS).unwrap();
    let list = DomainList::open(shared.clone(), own).await.unwrap();
    Node {
        list,
        me: me.peer_id(),
        shared,
        _keep: (net, backend),
    }
}

impl Node {
    async fn when_issued(&self, s: &InviteSecret, for_whom: Option<&str>) {
        let rec = issue_record(NOW, for_whom);
        Ledger::of(&self.list)
            .issue(&s.hash_hex(), &rec)
            .await
            .unwrap();
    }

    /// The parsed admit this node wrote for `subject`.
    async fn admit_of(&self, subject: PeerId) -> Option<AclValue> {
        let entry = self
            .shared
            .get(self.me, &admit_key(&subject))
            .await
            .unwrap()?;
        match parse_entry(&entry) {
            Parsed::Record { author, value, .. } if author == self.me => Some(value),
            other => panic!("the entry under the admit key is not this node's record: {other:?}"),
        }
    }

    async fn live_admits(&self) -> usize {
        let all = self.shared.scan(b"acl/admit/").await.unwrap();
        all.iter().filter(|e| !e.is_tombstone()).count()
    }
}
