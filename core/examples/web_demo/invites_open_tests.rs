use meshtown_core::web_gui::{ActionError, MemberError};

use super::OpenInvites;

fn mine_hash(o: &OpenInvites) -> String {
    o.list(true)
        .unwrap()
        .invites
        .iter()
        .find(|i| i.mine)
        .unwrap()
        .hash
        .clone()
}

fn other_hash(o: &OpenInvites) -> String {
    o.list(true)
        .unwrap()
        .invites
        .iter()
        .find(|i| !i.mine)
        .unwrap()
        .hash
        .clone()
}

#[test]
fn an_admin_sees_both_seeded_invites() {
    let r = OpenInvites::seeded().list(true).unwrap();
    assert_eq!(r.invites.len(), 2);
    let mine = r.invites.iter().find(|i| i.mine).unwrap();
    assert_eq!(mine.for_whom.as_deref(), Some("Anna, laptop"));
    let other = r.invites.iter().find(|i| !i.mine).unwrap();
    assert_eq!(other.for_whom, None);
}

#[test]
fn an_ordinary_member_is_refused() {
    let o = OpenInvites::seeded();
    assert!(matches!(
        o.list(false),
        Err(MemberError::Refused {
            code: "not_admin",
            ..
        })
    ));
}

#[test]
fn my_invite_can_be_renamed() {
    let mut o = OpenInvites::seeded();
    let h = mine_hash(&o);
    let r = o.relabel(true, &h, Some("Ann")).unwrap();
    let mine = r.invites.iter().find(|i| i.mine).unwrap();
    assert_eq!(mine.for_whom.as_deref(), Some("Ann"));
}

#[test]
fn another_admins_invite_is_not_issuer() {
    let mut o = OpenInvites::seeded();
    let h = other_hash(&o);
    assert!(matches!(
        o.relabel(true, &h, Some("x")),
        Err(MemberError::Refused {
            code: "not_issuer",
            ..
        })
    ));
}

#[test]
fn an_unknown_hash_is_not_found() {
    let mut o = OpenInvites::seeded();
    assert!(matches!(
        o.relabel(true, &"0".repeat(64), Some("x")),
        Err(MemberError::Action(ActionError::NotFound(_)))
    ));
}

#[test]
fn an_empty_note_clears_the_label() {
    let mut o = OpenInvites::seeded();
    let h = mine_hash(&o);
    let r = o.relabel(true, &h, Some("   ")).unwrap();
    assert_eq!(r.invites.iter().find(|i| i.mine).unwrap().for_whom, None);
}

#[test]
fn an_issued_invite_shows_up_as_mine_with_its_note() {
    let mut o = OpenInvites::seeded();
    o.issue(Some("Bob, phone"));
    let r = o.list(true).unwrap();
    assert_eq!(r.invites.len(), 3);
    assert!(r.invites[0].mine);
    assert_eq!(r.invites[0].for_whom.as_deref(), Some("Bob, phone"));
}
