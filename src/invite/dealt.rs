//! An issued invite: the string and the list of members who confirmed they saved the entry.
//!
//! What it does: stores the outcome of `Issued::commit` and turns the number of holders into a reach
//! flag (`Reach`) with ready texts for the API and the console.
//! What it doesn't do: doesn't go to the network and doesn't assemble the string; collecting holders is `invite::spread`.
//! Links: provided by `invite::Issued::commit`; read by `web::control::PendingInvite`, the handler of
//! `/api/network/invite` and `console::print_invite`.

use crate::net::PeerId;

/// An issued invite: the string and the members who confirmed the entry (holders).
#[derive(Clone)]
pub struct Dealt {
    pub text: String,
    pub holders: Vec<PeerId>,
}

/// Debug output without the invite string: it holds a one-time secret, which does not get into the log.
impl std::fmt::Debug for Dealt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dealt")
            .field("holders", &self.holders.len())
            .field("reach", &self.reach())
            .field("text_len", &self.text.chars().count())
            .finish()
    }
}

impl Dealt {
    /// The body of the `GET /api/network/invite` answer: the string, the number of holders and the reach.
    pub fn api_body(&self) -> serde_json::Value {
        serde_json::json!({
            "invite": self.text,
            "holders": self.holders.len(),
            "reach": self.reach().as_str(),
        })
    }

    /// An invite without holders (stubs and the "nobody online" case).
    pub fn without_holders(text: String) -> Dealt {
        Dealt { text, holders: Vec::new() }
    }

    /// The reach flag by the number of holders.
    pub fn reach(&self) -> Reach {
        Reach::of(self.holders.len())
    }
}

/// Who will let a newcomer in: only the creator, the creator and one more, or the entry has spread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    IssuerOnly,
    IssuerAndOne,
    Spread,
}

impl Reach {
    /// The flag by the number of holders: 0, 1, two and more.
    pub fn of(holders: usize) -> Reach {
        Reach::from_count(holders.min(2))
    }

    fn from_count(n: usize) -> Reach {
        [Reach::IssuerOnly, Reach::IssuerAndOne, Reach::Spread][n]
    }

    /// The word for the `reach` field in the API.
    pub fn as_str(self) -> &'static str {
        self.words().0
    }

    /// The line for the console.
    pub fn console_line(self) -> &'static str {
        self.words().1
    }

    fn words(self) -> (&'static str, &'static str) {
        match self {
            Reach::IssuerOnly => (
                "issuer_only",
                "only this node will let a newcomer in: no other members are online or they didn't confirm the entry — don't turn it off until the newcomer joins",
            ),
            Reach::IssuerAndOne => (
                "issuer_and_one",
                "this node and one more member will let a newcomer in — better not to turn this node off until the newcomer joins",
            ),
            Reach::Spread => (
                "spread",
                "the invite entry is held by two more members online — a newcomer will join even without this node",
            ),
        }
    }
}

#[cfg(test)]
#[path = "dealt_tests.rs"]
mod tests;
