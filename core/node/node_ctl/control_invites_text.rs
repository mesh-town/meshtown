//! The strings of the invites this node made: kept when an invite is issued, given back to an admin on
//! request. The strings are secrets: they go only into the reply body, never into the journal.

use std::path::PathBuf;

use crate::config::config_file::{keep, kept};
use crate::invite::store::Ledger;
use crate::invite::{Dealt, Issued};
use crate::node::output::say;
use crate::node::RunningNode;
use crate::transport::net_mainline::unix_now;
use crate::web_gui::{ActionError, InviteText, InviteTextFuture, MemberError};

use super::control_invites::{admins_of, internal};
use super::state::NodeCtl;

const GONE: &str = "this invite can no longer be shown";

/// Writes the issue record, then keeps the strings when `path` is set. A failed keep is a journal line
/// without the strings; the invite is still handed out.
pub(super) async fn commit_and_keep(
    issued: Issued,
    path: Option<PathBuf>,
) -> anyhow::Result<Dealt> {
    let (hash, made_unix) = (issued.hash().to_string(), issued.made_unix());
    let dealt = issued.commit().await?;
    if let Some(Err(e)) = path.map(|p| keep(&p, &hash, made_unix, &dealt)) {
        say("node", format!("invite not kept for later: {e}"));
    }
    Ok(dealt)
}

impl NodeCtl {
    pub(super) fn invite_text_impl(&self, hash: String) -> InviteTextFuture {
        match self.running_node_for_read() {
            Ok(node) => Box::pin(async move { text_of(&node, &hash).await }),
            Err(e) => Box::pin(async move { Err(MemberError::Action(e)) }),
        }
    }
}

fn gone() -> MemberError {
    MemberError::Action(ActionError::NotFound(GONE.to_string()))
}

/// The kept strings of our own open invite under `hash`; every other case is the same `gone` answer.
async fn text_of(node: &RunningNode, hash: &str) -> Result<InviteText, MemberError> {
    let (me, admins) = admins_of(node.access.view.verdicts().as_ref())?;
    let path = node.kept_invites_path.as_deref().ok_or_else(gone)?;
    let store = Ledger::of(&node.domains);
    let open = store
        .open(unix_now(), Some(&admins))
        .await
        .map_err(internal)?;
    let own = open.iter().any(|o| o.hash == hash && o.issuer == Some(me));
    let record = kept(path, hash).filter(|_| own).ok_or_else(gone)?;
    Ok(InviteText {
        text: record.text,
        qr_text: record.qr_text,
    })
}
