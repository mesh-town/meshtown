//! `NetBrief.excluded_by` and `excluded_reason`: who removed this node and the note they gave, from the roles of the slot.

use super::*;
use crate::web_gui::config_api::block_support::{main_slot, name, Net, Removal, ROOT};

fn brief(net: Net) -> NetBrief {
    brief_of(&main_slot(net), &NetSlotId::parse("main").unwrap())
}

#[test]
fn brief_carries_excluded_by_and_reason() {
    // given: node 3 was removed by the creator with a note
    let net = Net::new(3, &[ROOT])
        .removed(Removal::Votes { by: ROOT })
        .reason("spam");
    // when
    let b = brief(net);
    // then: the name of the author, as in `network.excluded_by.name`, and the note
    assert_eq!(b.excluded_by, Some(name(ROOT)));
    assert_eq!(b.excluded_reason.as_deref(), Some("spam"));
}

#[test]
fn removal_without_a_note_has_the_author_and_no_reason() {
    // given / when
    let b = brief(Net::new(3, &[ROOT]).removed(Removal::Votes { by: ROOT }));
    // then
    assert_eq!(b.excluded_by, Some(name(ROOT)));
    assert_eq!(b.excluded_reason, None);
}

#[test]
fn brief_of_a_left_node_has_no_excluded_by() {
    // given / when: node 3 left on its own
    let b = brief(Net::new(3, &[ROOT]).removed(Removal::Left).reason("spam"));
    // then: both are `null`
    assert_eq!((b.excluded_by, b.excluded_reason), (None, None));
}

#[test]
fn brief_of_a_member_has_neither() {
    // given / when
    let b = brief(Net::new(3, &[ROOT]));
    // then
    assert_eq!((b.excluded_by, b.excluded_reason), (None, None));
}
