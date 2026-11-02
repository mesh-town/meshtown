//! Watching the node's own admission when it started with the network key and an invite it did not spend.
//! The invite is spent only when the members turn this computer away (and this computer does not count itself
//! a member); a member keeps the invite. What is not here: spending the invite itself, that is
//! `NodeControl::join_by_invite`.

use std::time::Duration;

use crate::invite::{same_network, ParsedInvite, INVITE_TTL_SECS};
use crate::node::output::say;
use crate::web_gui::{now_unix, save_key_of, MembersReply, SlotHandle};

/// How often the verdicts are looked at.
pub(crate) const ADMIT_TICK: Duration = Duration::from_secs(5);

/// How the network sees this computer, from the member list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Standing {
    /// This computer counts itself a member.
    pub member: bool,
    /// A neighbour turned this computer away as not listed.
    pub unlisted: bool,
    /// The network excluded this computer.
    pub excluded: bool,
}

/// What to do with the invite now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Need {
    /// A member: the invite is kept.
    Member,
    /// Nothing is known yet.
    Wait,
    /// Turned away: spend the invite.
    Spend,
    /// The invite ran out first.
    Expired,
    /// The node works with another network.
    Elsewhere,
}

/// Reads the standing off the member list.
pub(crate) fn standing_of(reply: &MembersReply) -> Standing {
    let member = reply
        .members
        .iter()
        .any(|m| m.is_self && m.status == "member");
    let turned_away = |m: &crate::web_gui::MemberRow| {
        let refusal = m.last_refusal.as_ref();
        !m.is_self && refusal.is_some_and(|r| r.side == "them" && r.reason == "unlisted")
    };
    Standing {
        member,
        unlisted: !member && reply.members.iter().any(turned_away),
        excluded: reply.me_excluded,
    }
}

/// The decision: another network first, then a member, then the deadline, then a refusal; otherwise wait.
/// `None` — the node has published no verdicts yet: it is waited for like a node nobody has turned away.
pub(crate) fn invite_need(
    standing: Option<Standing>,
    on_invite_net: bool,
    now: u64,
    deadline: u64,
) -> Need {
    if !on_invite_net {
        return Need::Elsewhere;
    }
    let s = standing.unwrap_or(Standing {
        member: false,
        unlisted: false,
        excluded: false,
    });
    if s.member {
        return Need::Member;
    }
    if now >= deadline {
        return Need::Expired;
    }
    if s.excluded || s.unlisted {
        return Need::Spend;
    }
    Need::Wait
}

/// Whether the slot works with the invite's network (no key: it does not).
fn on_invite_net(slot: &SlotHandle, inv: &ParsedInvite) -> bool {
    let cfg = slot.config.lock().unwrap_or_else(|e| e.into_inner());
    let key = cfg.current_network_key.as_deref();
    key.is_some_and(|k| same_network(&inv.head, k).unwrap_or(false))
}

async fn standing_now(slot: &SlotHandle) -> Option<Standing> {
    let reply = slot.node.members().await.ok()?;
    Some(standing_of(&reply))
}

/// Looks at the verdicts every `tick` until it knows what to do with the invite, then does it once.
pub(crate) async fn watch_admission(slot: SlotHandle, inv: ParsedInvite, tick: Duration) {
    let deadline = inv.head.made_unix + INVITE_TTL_SECS;
    loop {
        let standing = standing_now(&slot).await;
        let need = invite_need(standing, on_invite_net(&slot, &inv), now_unix(), deadline);
        if need != Need::Wait {
            return settle(&slot, inv, need);
        }
        tokio::time::sleep(tick).await;
    }
}

fn settle(slot: &SlotHandle, inv: ParsedInvite, need: Need) {
    match need {
        Need::Member => say(
            "invite",
            "a member of this network; the invite was not spent",
        ),
        Need::Expired => say("invite", "the invite has run out; it was not spent"),
        Need::Elsewhere => say(
            "invite",
            "this node works with another network; the invite was not spent",
        ),
        Need::Spend => spend(slot, inv),
        Need::Wait => {}
    }
}

fn spend(slot: &SlotHandle, inv: ParsedInvite) {
    say(
        "invite",
        "the members turned this computer away; joining by invite",
    );
    let save = save_key_of(slot.config.clone(), true);
    if let Err(e) = slot.node.join_by_invite(inv.head, inv.peers, save) {
        say("invite", format!("joining by invite failed: {e:?}"));
    }
}

#[cfg(test)]
#[path = "admit_watch_tests.rs"]
mod tests;
