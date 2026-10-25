//! The built-but-not-issued invite and the step that writes its issue record.

use anyhow::{Context, Result};

use super::{build_keyed, hold_wire, known, spread, store, Dealt, InviteHead};
use crate::members::network_key::NetworkKey;
use crate::transport::addr_order::PrimaryIps;
use crate::transport::net::{NetStack, PeerAddr, PeerId};

/// A built but not yet issued invite: the string is handed out only after the issue record is written into
/// the network document (`commit`), otherwise members would not learn the secret.
pub struct Issued {
    /// Draft of the string without holders; the real string is given by `commit`.
    pub text: String,
    pub(super) ledger: store::Ledger,
    pub(super) hash: String,
    pub(super) record: store::IssueRecord,
    pub(super) head: InviteHead,
    pub(super) own_addr: PeerAddr,
    pub(super) others: Vec<PeerId>,
    pub(super) rest: Vec<PeerId>,
    pub(super) primary: PrimaryIps,
    pub(super) stack: NetStack,
    pub(super) key: NetworkKey,
}

impl Issued {
    /// Writes the issue record, collects the holders (members that saved their copy) and returns
    /// the string with them first; if writing the issue record fails — there is no string.
    pub async fn commit(self) -> Result<Dealt> {
        let issuer = self.head.kind.root().map(|_| self.own_addr.id);
        self.ledger
            .issue_as(&self.hash, &self.record, issuer)
            .await
            .context("writing the invite into the network document failed")?;
        let (list, holders) = self.invite_addrs().await;
        let text = build_keyed(&self.head, &list[0], &list[1..], &self.primary)?;
        Ok(Dealt { text, holders })
    }

    /// Asks the members to keep the invite and returns the addresses to put into the string (holders first)
    /// together with the holders.
    async fn invite_addrs(&self) -> (Vec<PeerAddr>, Vec<PeerId>) {
        let frame = hold_wire::hold_frame(
            &self.key,
            &self.head.secret.hash_bytes(),
            self.head.made_unix,
        );
        let asked = spread::candidates(&self.others, self.own_addr.id);
        let holders = spread::gather(&self.stack, frame, asked).await;
        let ordered = spread::ordered(&holders, &self.own_addr, &self.rest);
        let list = known::with_known(ordered, |id| self.stack.proven_addr(id));
        (list, holders)
    }
}
