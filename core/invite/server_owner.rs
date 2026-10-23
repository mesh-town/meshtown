//! The owner path of the invite server: a newcomer shows the invite secret and proves it owns its permanent key.
//!
//! What it does: checks the proof, asks the membership view whether this node may let the key in, places the
//! claim, waits out `CLAIM_SETTLE`, compares the rivals' claims, writes the newcomer's `admit` record and only
//! then burns the invite with a mark and hands out the network key. The first checks spend nothing.
//! What it doesn't do: no frame reading and no HOLD (`invite::server`); the record format is `access::record`.
//! Links: `InviteServer::answer` calls `owner_reply`; the proof is in `invite::proof`.

use std::time::Duration;

use tokio::time::{sleep, timeout};

use super::proof::{admit_owner, owner_proven};
use super::server::{log_refused, InviteServer, ANSWER_TIMEOUT, CLAIM_SETTLE};
use super::store::{settle, IssueVerdict, Ledger, Settle};
use super::wire::{
    code_from, encode_invite_reply, parse_owner_request, OwnerRequest, RedeemCode,
    OWNER_REQUEST_LEN, REPLY_LEN,
};
use crate::members::access::view::Deny;
use crate::members::network_key::NetworkKey;
use crate::node::output::{say, short_peer_id};
use crate::proxy::domains::DomainList;
use crate::transport::net::PeerId;
use crate::transport::net_mainline;

/// How long to wait for the membership view to be ready before answering "not ready".
const READY_WAIT: Duration = Duration::from_secs(1);

type Verdict = (RedeemCode, Option<NetworkKey>);

/// The refusal for the membership view's verdict on the owner; `None` — go on.
/// Refusals by this node's own vote or block are retryable: another holder may decide differently.
fn owner_code(r: Result<(), Deny>) -> Option<RedeemCode> {
    match r {
        Ok(()) | Err(Deny::Unlisted | Deny::Forgotten | Deny::RemovedByMe) => None,
        Err(Deny::Excluded) => Some(RedeemCode::Excluded),
        Err(why) => {
            say(
                "invite",
                format!(
                    "invite not given: this node refuses the newcomer ({})",
                    why.code()
                ),
            );
            Some(RedeemCode::NotReady)
        }
    }
}

impl InviteServer {
    /// The answer to an owner request. Nothing is claimed until the proof and the membership verdict pass.
    pub(super) async fn owner_reply(
        &self,
        raw: &[u8; OWNER_REQUEST_LEN],
        transport: PeerId,
        who: &str,
    ) -> [u8; REPLY_LEN] {
        let Some(req) = parse_owner_request(raw).filter(|r| owner_proven(r, transport)) else {
            log_refused(who, RedeemCode::BadRequest);
            return encode_invite_reply(RedeemCode::BadRequest, None);
        };
        let (code, key) = match timeout(ANSWER_TIMEOUT, self.decide_owner(&req)).await {
            Ok(done) => done,
            Err(_) => (RedeemCode::Internal, None),
        };
        log_owner(who, req.owner, code);
        encode_invite_reply(code, key.as_ref())
    }

    /// The membership verdict, then two phases: a claim under the lock, a wait without the lock, the comparison,
    /// the record and the mark under the lock.
    async fn decide_owner(&self, req: &OwnerRequest) -> Verdict {
        if let Some(code) = self.membership_refusal(req.owner).await {
            return (code, None);
        }
        let Some(list) = self.list.get() else {
            return (RedeemCode::NotReady, None);
        };
        let ledger = Ledger::of(list);
        let hash = req.secret.hash_hex();
        if let Err(code) = self.place_claim(&ledger, &hash).await {
            return (code, None);
        }
        sleep(CLAIM_SETTLE).await;
        self.settle_and_grant(list, &ledger, &hash, req.owner).await
    }

    /// Whether this node's view of the list lets the newcomer's key in.
    async fn membership_refusal(&self, owner: PeerId) -> Option<RedeemCode> {
        if timeout(READY_WAIT, self.access.ready()).await.is_err() {
            return Some(RedeemCode::NotReady);
        }
        owner_code(self.access.check(owner))
    }

    /// Phase 1: a verdict and one's own claim; if not `Grant` — its code.
    async fn place_claim(&self, ledger: &Ledger, hash: &str) -> Result<(), RedeemCode> {
        let _guard = self.gate.lock().await;
        let now_unix = net_mainline::unix_now();
        let verdict = ledger.verdict(hash, now_unix).await;
        if verdict != IssueVerdict::Grant {
            return Err(code_from(verdict));
        }
        ledger
            .claim(hash, now_unix)
            .await
            .map_err(|_| RedeemCode::Internal)
    }

    /// The newest forget vote about the owner in this node's view of the list.
    fn forget_mark(&self, owner: &PeerId) -> Option<u64> {
        self.access.verdicts()?.ledger.forget_mark(owner)
    }

    /// Phase 2: a fresh verdict, comparing claims (the one who yields gets `Busy`), the owner's record, the mark,
    /// the key. A failed record write leaves the invite unspent: the claim goes stale and the client retries.
    async fn settle_and_grant(
        &self,
        list: &DomainList,
        ledger: &Ledger,
        hash: &str,
        owner: PeerId,
    ) -> Verdict {
        let _guard = self.gate.lock().await;
        let now_unix = net_mainline::unix_now();
        let verdict = ledger.verdict(hash, now_unix).await;
        if verdict != IssueVerdict::Grant {
            return (code_from(verdict), None);
        }
        let Ok(rivals) = ledger.rival_claims(hash, now_unix).await else {
            return (RedeemCode::Internal, None);
        };
        if settle(ledger.me(), &rivals) == Settle::Yield {
            return (RedeemCode::Busy, None);
        }
        let after = self.forget_mark(&owner);
        if let Err(e) = admit_owner(list, hash, owner, after, now_unix).await {
            say("invite", format!("couldn't write the member record: {e:#}"));
            return (RedeemCode::Internal, None);
        }
        match ledger.mark_used(hash, now_unix).await {
            Ok(()) => (RedeemCode::Ok, Some(self.key.clone())),
            Err(_) => (RedeemCode::Internal, None),
        }
    }
}

fn log_owner(who: &str, owner: PeerId, code: RedeemCode) {
    if code == RedeemCode::Ok {
        say(
            "invite",
            format!("node {who} let in by invite as {}", short_peer_id(&owner)),
        );
        return;
    }
    log_refused(who, code);
}

#[cfg(test)]
#[path = "server_owner_forget_tests.rs"]
mod forget_tests;
