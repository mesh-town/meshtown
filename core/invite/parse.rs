//! Parsing an invite string back into `ParsedInvite`. Assembling the string is in `invite.rs`.

use anyhow::{anyhow, bail, Result};

use crate::members::network_key;
use crate::node::output::NET_NAME;

use super::{
    Invite, InviteHead, InviteSecret, ParsedInvite, INVITE_KEYED_VERSION, INVITE_PREFIX,
    INVITE_VERSION, MAX_INVITE_BYTES, MAX_INVITE_CHARS, OLD_INVITE_PREFIX,
};

/// The refusal text for an invite of the old format (with the network key inside).
const TEXT_OLD_INVITE: &str = "This invite is of the old format (mesh-invite1-): it contained the network key, \
    such invites are no longer accepted. Ask a network member for a new invite — from the updated program.";

/// Whether the string (after trimming whitespace) starts with `INVITE_PREFIX` or `OLD_INVITE_PREFIX` in
/// any case — without parsing and without checking for broken data, only telling a key from
/// an invite.
pub fn looks_like_invite(text: &str) -> bool {
    let trimmed = text.trim();
    has_prefix(trimmed, INVITE_PREFIX) || has_prefix(trimmed, OLD_INVITE_PREFIX)
}

fn has_prefix(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// Parses an invite string. The first check that fires, in order, gives the error;
/// broken entries and the label become notes in `ParsedInvite::warnings`, joining goes on.
pub fn parse(text: &str) -> Result<ParsedInvite> {
    let bytes = decode_invite_body(text)?;
    let invite: Invite = postcard::from_bytes(&bytes).map_err(|_| {
        anyhow!("invite is damaged: unreadable — copy the string in full once more")
    })?;
    if invite.peers.is_empty() {
        bail!("the invite has no member address at all — ask for a new one");
    }
    let (peers, mut warnings) = peers_from_roster(&invite.peers)?;
    let label = normalize_label_or_warn(invite.label, &mut warnings);
    Ok(ParsedInvite {
        head: InviteHead {
            secret: InviteSecret::from_bytes(invite.secret),
            network_id: invite.network_id,
            label,
            made_unix: invite.made_unix,
        },
        peers,
        warnings,
    })
}

/// Strips whitespace, checks the length, prefix and version, decodes the body from base32 — before parsing
/// with `postcard`. The first check that fires, in order, gives the error.
fn decode_invite_body(text: &str) -> Result<Vec<u8>> {
    let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        bail!("the invite string is empty");
    }
    if cleaned.chars().count() > MAX_INVITE_CHARS {
        bail!("the invite string is longer than {MAX_INVITE_CHARS} characters — this is not an invite");
    }
    let lower = cleaned.to_ascii_lowercase();
    if lower.starts_with(OLD_INVITE_PREFIX) {
        bail!(TEXT_OLD_INVITE);
    }
    if !lower.starts_with(INVITE_PREFIX) {
        bail!("this is not an invite: the string must start with {INVITE_PREFIX}");
    }
    let body = &lower[INVITE_PREFIX.len()..];
    if body.is_empty() {
        bail!(
            "invite is damaged: nothing after mesh-invite2- — copy the string in full \
            once more"
        );
    }
    let bytes = data_encoding::BASE32_NOPAD_NOCASE
        .decode(body.as_bytes())
        .map_err(|_| {
            anyhow!(
                "invite is damaged: the string has foreign characters or is cut off — copy it in \
            full once more"
            )
        })?;
    if bytes.len() > MAX_INVITE_BYTES {
        bail!("the invite is longer than {MAX_INVITE_BYTES} bytes — this is not an invite");
    }
    if bytes.is_empty() {
        bail!(
            "invite is damaged: nothing after mesh-invite2- — copy the string in full \
            once more"
        );
    }
    if bytes[0] != INVITE_VERSION && bytes[0] != INVITE_KEYED_VERSION {
        bail!(
            "invite of a different format ({}) — update the program",
            bytes[0]
        );
    }
    Ok(bytes)
}

/// Parses member entries into addresses; a broken entry becomes a note, not an error —
/// an error only if no address is left after parsing.
fn peers_from_roster(
    entries: &[crate::transport::net_mainline::record::RosterEntry],
) -> Result<(Vec<crate::transport::net::PeerAddr>, Vec<String>)> {
    let mut warnings = Vec::new();
    let mut peers = Vec::new();
    for entry in entries {
        match crate::transport::net_mainline::record::entry_to_addr(entry) {
            Ok(addr) => peers.push(addr),
            Err(e) => warnings.push(format!("member entry skipped: {e:#}")),
        }
    }
    if peers.is_empty() {
        bail!("none of the member addresses in the invite is readable — ask for a new one");
    }
    Ok((peers, warnings))
}

/// Normalizes the network label from the invite; a failed normalization is a note, not an error.
fn normalize_label_or_warn(raw: Option<String>, warnings: &mut Vec<String>) -> Option<String> {
    match raw {
        Some(raw) => match network_key::normalize_label(&raw) {
            Ok(normalized) => Some(normalized),
            Err(e) => {
                warnings.push(format!(
                    "network label {NET_NAME} from the invite skipped: {e:#}"
                ));
                None
            }
        },
        None => None,
    }
}
