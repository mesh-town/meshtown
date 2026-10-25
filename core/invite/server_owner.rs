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

    /// The view's verdict on the owner; in a Managed network an excluded key may come back by a new invite.
    fn checked(&self, owner: PeerId) -> Result<(), Deny> {
        match self.access.check(owner) {
            Err(Deny::Excluded) if self.kind.root().is_some() => Ok(()),
            other => other,
        }
    }

    /// Whether this node's view of the list lets the newcomer's key in.
    async fn membership_refusal(&self, owner: PeerId) -> Option<RedeemCode> {
        if timeout(READY_WAIT, self.access.ready()).await.is_err() {
            return Some(RedeemCode::NotReady);
        }
        if !self.may_serve() {
            return Some(RedeemCode::NotReady); // only an admin lets a newcomer in; another holder may
        }
        if self
            .access
            .verdicts()
            .is_some_and(|v| v.ledger.is_forgotten(&v.me) || v.ledger.excluded.contains_key(&v.me))
        {
            // the server itself is forgotten or excluded (the view never refuses its own key, so
            // this must be checked here); retryable: another holder can let the newcomer in
            return Some(RedeemCode::NotReady);
        }
        owner_code(self.checked(owner))
    }

    /// Phase 1: a verdict and one's own claim; if not `Grant` — its code.
    async fn place_claim(&self, ledger: &Ledger, hash: &str) -> Result<(), RedeemCode> {
        let _guard = self.gate.lock().await;
        let now_unix = net_mainline::unix_now();
        let verdict = ledger
            .verdict(hash, now_unix, self.record_authors().as_ref())
            .await;
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

    /// The time the owner's record must be later than: the newest forget vote, or in a Managed network
    /// the exclusion that a returning key outdates.
    fn rewrite_after(&self, owner: &PeerId) -> Option<u64> {
        let excluded = self.kind.root().and_then(|_| self.excluded_secs(owner));
        self.forget_mark(owner).max(excluded)
    }

    /// When the owner's exclusion was written, in seconds.
    fn excluded_secs(&self, owner: &PeerId) -> Option<u64> {
        let verdicts = self.access.verdicts()?;
        let exclusion = verdicts.ledger.excluded.get(owner)?;
        Some(exclusion.at_micros / 1_000_000)
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
        let admins = self.record_authors();
        let verdict = ledger.verdict(hash, now_unix, admins.as_ref()).await;
        if verdict != IssueVerdict::Grant {
            return (code_from(verdict), None);
        }
        let Ok(mut rivals) = ledger.rival_claims(hash, now_unix).await else {
            return (RedeemCode::Internal, None);
        };
        rivals.retain(|r| admins.as_ref().is_none_or(|set| set.contains(r)));
        if settle(ledger.me(), &rivals) == Settle::Yield {
            return (RedeemCode::Busy, None);
        }
        let after = self.rewrite_after(&owner);
        if let Err(e) = admit_owner(list, hash, owner, after, now_unix).await {
            say("invite", format!("couldn't write the member record: {e:#}"));
            return (RedeemCode::Internal, None);
        }
        match ledger.mark_used(hash, now_unix).await {
            Ok(()) => (RedeemCode::Ok, Some(self.reply_secret.clone())),
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

#[cfg(test)]
#[path = "server_owner_forget_more_tests.rs"]
mod forget_more_tests;

#[cfg(test)]
#[path = "server_owner_excluded_tests.rs"]
mod excluded_tests;
