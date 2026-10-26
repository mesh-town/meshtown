//! Tests of the flags of what a node may do (`abilities.rs`): an administrator, the last
//! administrator and a plain member of an administered network.

// assumed: this file is declared in `abilities.rs` as
// `#[cfg(test)] #[path = "abilities_tests.rs"] mod tests;`.

use std::sync::Arc;

use crate::members::access::abilities::{abilities, admins_of, Abilities};
use crate::members::access::authority::{Power, PowerIn};
use crate::members::access::record::Via;
use crate::members::access::tally::{tally, Ledger, TallyInput};
use crate::members::access::view::Verdicts;
use crate::transport::net::contract::pid;
use crate::transport::net::PeerId;

mod managed_network {
    use super::*;

    #[test]
    fn an_administrator_may_do_everything() {
        // given: the root and a second administrator; I am the root
        let ledger = managed_ledger(pid(1), &[pid(2)], &[]);
        // when / then
        assert_eq!(
            abilities(&verdicts(pid(1), ledger)),
            flags(true, true, true, true, true)
        );
    }

    #[test]
    fn the_last_administrator_may_do_everything_but_leave() {
        // given: the root alone is the administrator
        let ledger = managed_ledger(pid(1), &[], &[pid(3)]);
        // when / then
        assert_eq!(
            abilities(&verdicts(pid(1), ledger)),
            flags(true, true, true, true, false)
        );
    }

    #[test]
    fn a_plain_member_may_only_leave() {
        // given: the root and the plain member 3; I am the plain member
        let ledger = managed_ledger(pid(1), &[], &[pid(3)]);
        // when / then
        assert_eq!(
            abilities(&verdicts(pid(3), ledger)),
            flags(false, false, false, false, true)
        );
    }

    #[test]
    fn it_lists_the_root_and_the_granted_administrators() {
        // given: the root 1 and the administrators 4 and 2
        let verdicts = verdicts(pid(1), managed_ledger(pid(1), &[pid(4), pid(2)], &[]));
        // when
        let mut listed = admins_of(&verdicts);
        // then
        let mut expected: Vec<String> = [pid(1), pid(2), pid(4)]
            .iter()
            .map(|p| p.to_string())
            .collect();
        listed.sort();
        expected.sort();
        assert_eq!(listed, expected);
    }
}

// --- helpers ---

fn flags(invite: bool, remove: bool, set_admins: bool, delete: bool, leave: bool) -> Abilities {
    Abilities {
        can_invite: invite,
        can_remove: remove,
        can_set_admins: set_admins,
        can_delete_network: delete,
        can_leave: leave,
    }
}

fn verdicts(me: PeerId, ledger: Ledger) -> Verdicts {
    Verdicts {
        me,
        ledger: Arc::new(ledger),
        blocks: Default::default(),
    }
}

/// An administered network of `root`: the `admins` were admitted and granted the role by the root, the
/// `members` were only admitted. Seen by the root.
fn managed_ledger(root: PeerId, admins: &[PeerId], members: &[PeerId]) -> Ledger {
    let everyone = admins.iter().chain(members);
    let mut input = TallyInput {
        me: root,
        root,
        votes: vec![],
        powers: everyone
            .map(|p| power(root, Power::Admit(Via::Invite), *p, 1))
            .collect(),
    };
    input
        .powers
        .extend(admins.iter().map(|p| power(root, Power::Grant, *p, 2)));
    tally(&input)
}

fn power(author: PeerId, power: Power, subject: PeerId, at: u64) -> PowerIn {
    PowerIn {
        author,
        power,
        subject,
        ts_micros: at * 1_000_000,
    }
}
