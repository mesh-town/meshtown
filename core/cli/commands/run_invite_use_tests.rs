//! What `settle_invite` reports about the invite: kept for later, or spent. The report decides whether the
//! node watches its own admission after the start.

use super::persisting_the_key::{run_with_file, Dir};
use super::*;

#[test]
fn a_kept_invite_is_watched() {
    let inv = invite_of_net_a();
    let watched = InviteUse::Kept(inv.clone()).to_watch().expect("watched");
    assert_eq!(watched.head.secret, inv.head.secret);
    assert_eq!(watched.peers, inv.peers);
}

#[test]
fn a_redeemed_invite_is_not_watched() {
    assert!(InviteUse::Redeemed(invite_of_net_a()).to_watch().is_none());
}

#[tokio::test]
async fn settling_a_key_of_the_same_network_keeps_the_invite_for_later() {
    // given: a settings file that already holds the key of the invite's network
    let dir = Dir::new("kept");
    let mut run = run_with_file(&dir);
    persist_key(&mut run, &text_a(), NOW).unwrap();
    let path = run.file_report.path.clone().expect("the file path");
    let before = std::fs::read(&path).unwrap();

    // when
    let used = settle_invite(Some(invite_of_net_a()), None, &mut run, &[], NOW)
        .await
        .unwrap();

    // then: the invite comes back as kept, and the file is untouched
    let Some(InviteUse::Kept(p)) = used else {
        panic!("the invite must be kept, got {used:?}");
    };
    assert_eq!(p.head.secret, secret());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
