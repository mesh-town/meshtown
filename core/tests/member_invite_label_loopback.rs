//! The "for whom" label of an invite from the issue to the member list, over real nodes on loopback.
//!
//! What it does: in a Managed network the creator issues an invite with a label, renames it, a second admin
//! tries to rename it, a newcomer redeems it; the open list is read the way `node_ctl` builds it, and the member
//! rows are built by the builder of `GET /api/members` for an admin and for a plain member as the viewer.
//! What it does not do: no web layer (the refusal codes of the page are unit tests of `node_ctl`); every wait has
//! a ceiling so a hang fails the test.

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use meshtown_core::invite::open::{OpenInvite, Relabel};
use meshtown_core::invite::store::Ledger;
use meshtown_core::invite::{self, ParsedInvite};
use meshtown_core::members::access::actions::{apply, MemberAction};
use meshtown_core::members::access::report::members_reply;
use meshtown_core::members::access::view::Verdicts;
use meshtown_core::node::RunningNode;
use meshtown_core::transport::net::PeerId;
use meshtown_core::transport::net_mainline::unix_now;
use meshtown_core::web_gui::MemberRow;

#[path = "common/invite_nodes.rs"]
mod common;
#[path = "common/managed_net.rs"]
mod managed;
#[path = "common/invite_mesh.rs"]
mod mesh;

use managed::{
    redeem_as, start_joined, wait_both_members, wait_can_invite, ManagedNet, VIEW_LIMIT,
};
use mesh::source_of;

const MEMBER_SEED: u8 = 5;
const NEWCOMER_SEED: u8 = 6;

/// Issues an invite with a label, like the page does: `make` then `commit`.
async fn issue_labelled(node: &RunningNode, label: &str) -> ParsedInvite {
    let issued = invite::make(&source_of(node), Some(label)).expect("make must assemble a string");
    let dealt = issued.commit().await.expect("commit must write the issue");
    invite::parse(&dealt.text).expect("the issued string must parse")
}

/// The verdicts of a fresh recompute of the node's access view (`view` itself is crate-private).
async fn verdicts_of(node: &RunningNode) -> Arc<Verdicts> {
    node.access
        .refresh()
        .await
        .expect("the access view must recompute")
}

/// The admins whose records count.
async fn admins_of(node: &RunningNode) -> BTreeSet<PeerId> {
    verdicts_of(node).await.ledger.admins.clone()
}

/// The open invites the way `node_ctl` lists them for the node (an admin).
async fn open_invites(node: &RunningNode) -> Vec<OpenInvite> {
    let admins = admins_of(node).await;
    let store = Ledger::of(&node.domains);
    let got = store.open(unix_now(), Some(&admins)).await;
    got.expect("the open list must be read")
}

/// Polls `open_invites` of the node until `cond` holds for the list; false when the ceiling passes.
async fn wait_open(node: &RunningNode, cond: impl Fn(&[OpenInvite]) -> bool) -> bool {
    let met = || async { cond(&open_invites(node).await).then_some(()) };
    poll(met).await.is_some()
}

/// The row of `subject` in the member list as `viewer` builds it.
async fn row_seen_by(viewer: &RunningNode, subject: &RunningNode) -> Option<MemberRow> {
    let verdicts = verdicts_of(viewer).await;
    let reply = members_reply(&verdicts, &viewer.members, "", &[], None);
    let id = subject.id.to_string();
    reply.members.into_iter().find(|r| r.id_full == id)
}

/// Polls `check` until it gives `Some`; `None` when the ceiling passes.
async fn poll<T, F: Future<Output = Option<T>>>(mut check: impl FnMut() -> F) -> Option<T> {
    let deadline = Instant::now() + VIEW_LIMIT;
    loop {
        if let Some(got) = check().await {
            return Some(got);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The creator and a plain member that joined by the creator's invite, both listed.
async fn joined_member() -> (ManagedNet, RunningNode, RunningNode) {
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    let parsed = issue_labelled(&root, "First").await;
    let answer = redeem_as(MEMBER_SEED, &parsed.head, &parsed.peers)
        .await
        .expect("the creator's invite must be redeemed");
    let member = start_joined(&parsed.head, &answer, MEMBER_SEED, parsed.peers.clone()).await;
    wait_both_members(&member, &root).await;
    (net, root, member)
}

fn labelled(list: &[OpenInvite], label: &str) -> usize {
    let same = |o: &&OpenInvite| o.for_whom.as_deref() == Some(label);
    list.iter().filter(same).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn label_set_at_issue_is_listed_with_the_issuer() {
    // given: the creator is up and can invite
    let net = ManagedNet::new();
    let root = net.start_root(net.base.clone()).await;
    wait_can_invite(&root).await;
    // when: it issues an invite labelled "Anna"
    let parsed = issue_labelled(&root, "Anna").await;
    // then: the open list holds exactly that invite, labelled, with the creator as the issuer
    let list = open_invites(&root).await;
    assert_eq!(labelled(&list, "Anna"), 1, "list: {list:?}");
    let hash = parsed.head.secret.hash_hex();
    let ours = list.iter().find(|o| o.hash == hash).expect("our invite");
    assert_eq!(ours.issuer, Some(root.id));
    root.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn issuer_renames_and_the_admin_list_follows() {
    // given: a labelled invite of the creator and a member made an admin who sees it
    let (_net, root, member) = joined_member().await;
    let granted = apply(&root.access, MemberAction::Grant(member.id)).await;
    assert_eq!(
        granted,
        Ok(true),
        "the creator must make the member an admin"
    );
    wait_can_invite(&member).await;
    let parsed = issue_labelled(&root, "Anna").await;
    let hash = parsed.head.secret.hash_hex();
    assert!(
        wait_open(&member, |l| labelled(l, "Anna") == 1).await,
        "the second admin did not see the invite"
    );
    // when: the creator renames it
    let admins = admins_of(&root).await;
    let store = Ledger::of(&root.domains);
    let done = store.relabel(&hash, Some("Anna K"), unix_now(), Some(&admins));
    // then: it is done and the list follows
    assert_eq!(done.await.unwrap(), Relabel::Done);
    let list = open_invites(&root).await;
    assert_eq!((labelled(&list, "Anna K"), labelled(&list, "Anna")), (1, 0));
    // and: the second admin is not the issuer and is refused, the label stays
    assert!(
        wait_open(&member, |l| labelled(l, "Anna K") == 1).await,
        "the second admin did not see the new label"
    );
    let admins = admins_of(&member).await;
    let other = Ledger::of(&member.domains);
    let got = other.relabel(&hash, Some("Mine"), unix_now(), Some(&admins));
    assert_eq!(got.await.unwrap(), Relabel::NotIssuer);
    let list = open_invites(&root).await;
    assert_eq!(labelled(&list, "Anna K"), 1, "list: {list:?}");
    member.shutdown().await;
    root.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn redeemed_invite_leaves_the_list_and_labels_the_member_for_admins_only() {
    // given: a member and an invite of the creator renamed to "Anna K"
    let (_net, root, member) = joined_member().await;
    let parsed = issue_labelled(&root, "Anna").await;
    let hash = parsed.head.secret.hash_hex();
    let admins = admins_of(&root).await;
    let store = Ledger::of(&root.domains);
    let done = store.relabel(&hash, Some("Anna K"), unix_now(), Some(&admins));
    assert_eq!(done.await.unwrap(), Relabel::Done);
    assert_eq!(labelled(&open_invites(&root).await, "Anna K"), 1);
    // when: the newcomer redeems it and joins
    let answer = redeem_as(NEWCOMER_SEED, &parsed.head, &parsed.peers)
        .await
        .expect("the invite must be redeemed");
    let peers = parsed.peers.clone();
    let newcomer = start_joined(&parsed.head, &answer, NEWCOMER_SEED, peers).await;
    wait_both_members(&newcomer, &root).await;
    // then: the invite left the open list
    assert!(
        wait_open(&root, |l| l.iter().all(|o| o.hash != hash)).await,
        "the redeemed invite is still open"
    );
    // and: the creator's member list shows the newcomer as invited as "Anna K"
    let as_admin = || async { row_seen_by(&root, &newcomer).await?.invited_as };
    let shown = poll(as_admin).await;
    assert_eq!(shown.as_deref(), Some("Anna K"), "the admin sees {shown:?}");
    // and: the plain member's ledger, once it holds the note, hides it from the row and the label too
    let synced = || async { holds_note(&member, &newcomer).await.then_some(()) };
    assert!(
        poll(synced).await.is_some(),
        "the member's replica did not converge"
    );
    let row = row_seen_by(&member, &newcomer).await.expect("the row");
    assert_eq!(row.invited_as, None);
    assert_eq!(row.admission.and_then(|a| a.label), None);
    newcomer.shutdown().await;
    member.shutdown().await;
    root.shutdown().await;
}

/// True once the viewer's own ledger holds the note for the subject, so a hidden note is hidden by the rule
/// and not because it has not arrived yet.
async fn holds_note(viewer: &RunningNode, subject: &RunningNode) -> bool {
    let v = verdicts_of(viewer).await;
    v.ledger.invited_as.contains_key(&subject.id)
}
