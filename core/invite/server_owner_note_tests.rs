//! Test of the label the invite server's owner path puts into the owner's `admit` record in a managed network:
//! it comes from the admins' issue records only, never from a plain member's newer record.
//! The fixture and the helpers are in `server_owner_support.rs`.

use super::support::*;
use super::*;
use crate::invite::store::issue_key;
use crate::invite::store_admins::issue_value;
use crate::invite::wire::RedeemReply;
use crate::members::access::record::{admit_key, parse_entry, Parsed};
use crate::members::access::tally::tally;
use crate::transport::net::facade::test_support::{pid, secret};
use crate::transport::net::ReplicaEntry;
use bytes::Bytes;

#[tokio::test]
async fn the_admit_of_a_guest_carries_the_admins_label_not_a_members() {
    // given: a managed network (the server is the admin); the admin's record "Alice" and a newer one of a
    // plain member "Mallory" under the same invite
    let access = access_with(tally(&input_of(&[me(), owner_id()])));
    let node = setup_managed_node(access).await;
    node.when_issued_by(me(), now() - 100, "Alice");
    node.when_issued_by(pid(5), now() - 10, "Mallory");
    // when: the guest presents the invite
    let reply = node.when_owner_asks(&secret(7)).await;
    // then: the guest got the key and its admit record says "Alice"
    assert!(matches!(reply, Some(RedeemReply::Key(_))), "{reply:?}");
    assert_eq!(node.admit_note(owner_id()).await, "Alice");
}

impl Node {
    /// An issue record of the invite as arrived from `author`, who names itself as the issuer.
    fn when_issued_by(&self, author: PeerId, made_unix: u64, who: &str) {
        let rec = crate::invite::store::issue_record(made_unix, Some(who));
        let value = issue_value(&rec, Some(author)).unwrap();
        self.mem.put_raw(ReplicaEntry {
            author,
            key: issue_key(&issued().hash_hex()),
            value: Bytes::from(value),
            ts_micros: made_unix * 1_000_000,
        });
    }

    /// The note of the server's own admit record of `subject`.
    async fn admit_note(&self, subject: PeerId) -> String {
        let entry = self.shared.get(me(), &admit_key(&subject)).await.unwrap();
        match parse_entry(&entry.expect("no admit record")) {
            Parsed::Record { value, .. } => value.note,
            other => panic!("not a record under the admit key: {other:?}"),
        }
    }
}
