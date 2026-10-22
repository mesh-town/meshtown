//! Proof that the newcomer owns the permanent node key, and the admission record written for it.
//!
//! What it does: builds and checks the signature over the invite hash, the one-time transport id and the
//! owner id; takes the "for whom" label from the issue records; writes the `admit` record of the owner.
//! What it doesn't do: no network and no frames (`invite::wire`), no decision whether to let the owner in
//! (`access::view`).
//! Links: signed by `invite::redeem`, checked and written by `invite::server`.

use super::store::{IssueRecord, FOR_WHOM_MAX_CHARS, ISSUE_PREFIX};
use super::wire::OwnerRequest;
use super::InviteSecret;
use crate::access::record::{put, AclKind, AclValue, Via};
use crate::domains::DomainList;
use crate::net::{live, NetError, NodeSecret, PeerId, ReplicaEntry};

/// Domain separation of the owner proof; a changed byte here is a changed protocol.
pub const CTX_INVITE_OWNER: &[u8] = b"meshtown invite owner proof v1\0";

/// The signed message: context, invite hash, the one-time transport id, the owner id.
pub fn owner_message(hash: &[u8; 32], transport: PeerId, owner: PeerId) -> Vec<u8> {
    let mut msg = Vec::with_capacity(CTX_INVITE_OWNER.len() + 96);
    msg.extend_from_slice(CTX_INVITE_OWNER);
    msg.extend_from_slice(hash);
    msg.extend_from_slice(transport.as_bytes());
    msg.extend_from_slice(owner.as_bytes());
    msg
}

/// The newcomer's signature; `transport` is the id of its one-time transport.
pub fn sign_owner(owner: &NodeSecret, s: &InviteSecret, transport: PeerId) -> [u8; 64] {
    owner.sign(&owner_message(&s.hash_bytes(), transport, owner.peer_id()))
}

/// Whether the request is signed by the owner it names, for this very transport.
pub fn owner_proven(req: &OwnerRequest, transport: PeerId) -> bool {
    let msg = owner_message(&req.secret.hash_bytes(), transport, req.owner);
    req.owner.verify(&msg, &req.sig)
}

/// The "for whom" label of an invite: the newest readable issue record that has one, cut to 64 characters.
/// Copies kept by holders carry no label, so they are skipped.
pub fn issue_note(entries: &[ReplicaEntry]) -> String {
    let labelled = entries.iter().filter(|e| !e.is_tombstone()).filter_map(|e| {
        let rec: IssueRecord = postcard::from_bytes(&e.value).ok()?;
        Some((rec.made_unix, rec.for_whom?))
    });
    let newest = labelled.max_by_key(|(made, _)| *made);
    newest.map(|(_, who)| who.chars().take(FOR_WHOM_MAX_CHARS).collect()).unwrap_or_default()
}

/// Writes the owner's `admit` record (via the invite) into the shared document, with the invite's label.
pub async fn admit_owner(list: &DomainList, hash: &str, owner: PeerId, now_unix: u64) -> anyhow::Result<()> {
    let shared = list.shared_replica();
    let key = format!("{ISSUE_PREFIX}{hash}").into_bytes();
    let found = shared.scan(&key).await.map_err(NetError::into_anyhow)?;
    let issued: Vec<ReplicaEntry> = live(found).into_iter().filter(|e| e.key == key).collect();
    let value = AclValue { kind: AclKind::Admit, subject: owner, via: Via::Invite, made_unix: now_unix, note: issue_note(&issued) };
    put(&*shared, &value).await.map_err(NetError::into_anyhow)
}

#[cfg(test)]
#[path = "proof_tests.rs"]
mod tests;
