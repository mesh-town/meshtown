//! Network invite string: building it by the console command `invite` and parsing the `--invite` flag.
//!
//! What it does: builds a `mesh-invite2-…` string (one-time secret, network marker, label, build
//! time, up to five member addresses) and parses it back; writing the issue record into the network
//! document is done by `Issued::commit`.
//! What it doesn't do: the string holds no network key — a member hands it over via `invite::wire` after
//! the secret is checked; storage and network are in `invite::store`, `invite::server`, `invite::redeem`.
//! Links: takes its own address from the stack (`own_addr`), knows the other members through `membership::Members` (keys only:
//! dialing goes by key); a member record is `net_mainline::record::RosterEntry`; called by `console` (the `invite` command) and
//! `main.rs` (the `--invite` flag).

use std::collections::{BTreeSet, HashSet};

use anyhow::{bail, Context, Result};
use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};

use crate::members::membership::Members;
use crate::members::network_key::{NetKind, NetworkKey};
use crate::node::output::NET_NAME;
use crate::transport::addr_order::{truncate_ranked, PrimaryIps};
use crate::transport::net::{NetStack, PeerAddr, PeerId};
use crate::transport::net_mainline;
use crate::transport::net_mainline::record::{entry_from_addr, RosterEntry};

mod dealt;
mod head;
pub mod hold_wire;
mod issuer;
mod known;
mod parse;
pub mod proof;
pub mod redeem;
mod redeem_round;
pub mod server;
mod server_owner;
mod spread;
pub mod store;
mod store_admins;
pub mod wire;
pub use dealt::{Dealt, Reach};
pub use head::{check_fresh, hash_hex_of, same_network, InviteHead, InviteSecret, INVITE_TTL_SECS};
pub use issuer::NotAdmin;
pub use parse::{looks_like_invite, parse, TEXT_V2_INVITE};

/// Prefix of the invite string; carries the format number.
pub const INVITE_PREFIX: &str = "mesh-invite2-";
/// Prefix of the earlier invite kind (with the network key inside): recognized but not accepted.
pub const OLD_INVITE_PREFIX: &str = "mesh-invite1-";
/// How many members (counting the inviter) fit in one invite.
pub const MAX_INVITE_PEERS: usize = 5;
/// How many direct addresses a record without relay holds.
pub const MAX_DIRECT_ADDRS: usize = 2;
/// Limit of the invite string length in characters.
pub const MAX_INVITE_CHARS: usize = 4_000;
/// Limit of the invite size in bytes (before base32).
pub const MAX_INVITE_BYTES: usize = 2_000;

/// Format number of the invite: a member record without relay and addresses is legal and means
/// "dial by key".
const INVITE_KEYED_VERSION: u8 = 3;

/// Format number of the invite of a Managed network: the v3 body plus the network kind and creator.
const INVITE_MANAGED_VERSION: u8 = 4;

/// The `kind` byte of a Managed network in a version 4 invite.
pub const INVITE_KIND_MANAGED: u8 = 1;

/// Internal form of the invite before base32 — the field order is the format, it must not change.
#[derive(Serialize, Deserialize)]
struct Invite {
    version: u8,
    network_id: [u8; 4],
    label: Option<String>,
    secret: [u8; 32],
    made_unix: u64,
    peers: Vec<RosterEntry>,
}

/// Internal form of a version 4 invite: the version 3 body (its `version` is 4), then the network kind
/// and the 32 bytes of the network creator.
#[derive(Serialize, Deserialize)]
struct InviteV4 {
    body: Invite,
    kind: u8,
    root: [u8; 32],
}

/// A parsed invite.
#[derive(Clone, Debug)]
pub struct ParsedInvite {
    pub head: InviteHead,
    pub peers: Vec<PeerAddr>,
    pub warnings: Vec<String>,
}

/// Everything a live node needs to build an invite for itself (the console command `invite`).
#[derive(Clone)]
pub struct Source {
    pub stack: NetStack,
    pub members: Members,
    pub key: NetworkKey,
    /// The kind of the network the node is in.
    pub kind: NetKind,
    /// The secret an invite redemption answers with; equals `key` in a Peer network.
    pub reply_secret: NetworkKey,
    pub label: Option<String>,
    /// The node's domain list: provides the network's shared document and the author for issue records.
    pub domains: crate::proxy::domains::DomainList,
}

/// With relay direct addresses are not needed, without relay — at most `MAX_DIRECT_ADDRS`;
/// first come the default-route address and public ones, the foreign tunnel address (100.64/10) —
/// last (`addr_order`), but it stays if there is otherwise neither a route nor a public address;
/// a record with no relay and no addresses is legal and means "dial by key".
fn record_for_addr(addr: &PeerAddr, made_unix: u64, primary: &PrimaryIps) -> RosterEntry {
    let mut entry = entry_from_addr(addr, made_unix, primary);
    if entry.relay.is_some() {
        entry.addrs.clear();
    } else {
        truncate_ranked(&mut entry.addrs, MAX_DIRECT_ADDRS, primary);
    }
    entry
}

/// Builds an invite string: own address first, then members, repeats by node key are dropped, at most
/// `MAX_INVITE_PEERS` records; records without hints are kept, an invite of keys alone is legal —
/// `hyper` (like `mem` in tests) dials by key.
pub fn build_keyed(
    head: &InviteHead,
    me: &PeerAddr,
    others: &[PeerAddr],
    primary: &PrimaryIps,
) -> Result<String> {
    let mut seen: HashSet<[u8; 32]> = HashSet::new();
    let mut peers: Vec<RosterEntry> = Vec::new();
    for addr in std::iter::once(me).chain(others.iter()) {
        if peers.len() >= MAX_INVITE_PEERS {
            break;
        }
        let id_bytes = *addr.id.as_bytes();
        if !seen.insert(id_bytes) {
            continue;
        }
        peers.push(record_for_addr(addr, head.made_unix, primary));
    }
    let root = head.kind.root();
    let body = Invite {
        version: root.map_or(INVITE_KEYED_VERSION, |_| INVITE_MANAGED_VERSION),
        network_id: head.network_id,
        label: head.label.clone(),
        secret: *head.secret.as_bytes(),
        made_unix: head.made_unix,
        peers,
    };
    let bytes = match root {
        None => postcard::to_allocvec(&body),
        Some(root) => postcard::to_allocvec(&InviteV4 {
            body,
            kind: INVITE_KIND_MANAGED,
            root: *root.as_bytes(),
        }),
    }
    .context("could not write postcard")?;
    if bytes.len() > MAX_INVITE_BYTES {
        bail!(
            "invite did not fit: {} bytes with a limit of {MAX_INVITE_BYTES}",
            bytes.len()
        );
    }
    Ok(format!(
        "{INVITE_PREFIX}{}",
        BASE32_NOPAD.encode(&bytes).to_ascii_lowercase()
    ))
}

/// A built but not yet issued invite: the string is handed out only after the issue record is written into
/// the network document (`commit`), otherwise members would not learn the secret.
pub struct Issued {
    /// Draft of the string without holders; the real string is given by `commit`.
    pub text: String,
    ledger: store::Ledger,
    hash: String,
    record: store::IssueRecord,
    head: InviteHead,
    own_addr: PeerAddr,
    others: Vec<PeerId>,
    rest: Vec<PeerId>,
    primary: PrimaryIps,
    stack: NetStack,
    key: NetworkKey,
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
        let frame = hold_wire::hold_frame(
            &self.key,
            &self.head.secret.hash_bytes(),
            self.head.made_unix,
        );
        let asked = spread::candidates(&self.others, self.own_addr.id);
        let holders = spread::gather(&self.stack, frame, asked).await;
        let ordered = spread::ordered(&holders, &self.own_addr, &self.rest);
        let list = known::with_known(ordered, |id| self.stack.proven_addr(id));
        let text = build_keyed(&self.head, &list[0], &list[1..], &self.primary)?;
        Ok(Dealt { text, holders })
    }
}

/// Own address for an invite and a snapshot: the node key and the hints of all stack transports, merged in order.
pub(crate) fn own_addr(stack: &NetStack, me: PeerId) -> PeerAddr {
    let mut addr = PeerAddr::new(me);
    stack.invite_addrs().iter().for_each(|a| addr.merge(a));
    addr
}

/// Listed members the view lets in: the string's tail may name members that have no connection right now.
fn listed_members(src: &Source) -> BTreeSet<PeerId> {
    let Some(verdicts) = src.members.access().verdicts() else {
        return Default::default();
    };
    let allowed = |p: &&PeerId| verdicts.check(**p).is_ok();
    verdicts
        .ledger
        .admitted
        .iter()
        .filter(allowed)
        .copied()
        .collect()
}

/// Whom to ask to keep the invite: the connected members in a Peer network, the listed admins in a Managed one.
fn holders_pool(connected: &[PeerId], admins: Option<BTreeSet<PeerId>>) -> Vec<PeerId> {
    admins.map_or_else(|| connected.to_vec(), |set| set.into_iter().collect())
}

/// Builds the invite of a live node: own address plus the listed members (keys only; v3 dials by key).
/// Does not touch the document — `Issued::commit` does that.
pub fn make(src: &Source, for_whom: Option<&str>) -> Result<Issued> {
    if src.stack.is_closed() {
        bail!("node was restarted into another network {NET_NAME} — the invite command is unavailable until the process restarts");
    }
    let admins = issuer::issuing_admins(src)?;
    let connected: Vec<PeerId> = src.members.ids().into_iter().collect();
    let others = holders_pool(&connected, admins);
    let me_id = src.stack.me();
    let rest = spread::rest(&connected, &listed_members(src), me_id);
    let bare: Vec<PeerAddr> = rest.iter().copied().map(PeerAddr::new).collect();
    let rest_addr = known::with_known(bare, |id| src.stack.proven_addr(id));
    let head = InviteHead {
        secret: InviteSecret::generate()?,
        network_id: src.key.subkeys().network_id,
        label: src.label.clone(),
        made_unix: net_mainline::unix_now(),
        kind: src.kind,
    };
    let me = own_addr(&src.stack, src.stack.me());
    let primary = PrimaryIps::probe();
    let text = build_keyed(&head, &me, &rest_addr, &primary)?;
    Ok(Issued {
        text,
        ledger: store::Ledger::of(&src.domains),
        hash: head.secret.hash_hex(),
        record: store::issue_record(head.made_unix, for_whom),
        head,
        own_addr: me,
        others,
        rest,
        primary,
        stack: src.stack.clone(),
        key: src.key.clone(),
    })
}

#[cfg(test)]
#[path = "invite_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "invite_golden_tests.rs"]
mod golden_tests;

#[cfg(test)]
#[path = "invite_v3_tests.rs"]
mod v3_tests;

#[cfg(test)]
#[path = "invite_holders_format_tests.rs"]
mod holders_format_tests;
