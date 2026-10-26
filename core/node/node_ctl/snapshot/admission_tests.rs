//! The rule "this computer is kept out" (`admission.rs`): not excluded or pending, no other member
//! online, and at least one member refused it for a reason other than "unlisted". The reply is built by hand;
//! `me` is the node with seed 1 and is always online.

use super::admission_refused;
use crate::transport::net::contract::pid;
use crate::web_gui::{Counts, MemberRow, MembersReply, RefusalInfo};

mod my_own_status {
    use super::*;

    #[test]
    fn excluded_me_is_not_a_refusal() {
        // given: a member that refuses me, and my own exclusion
        let reply = reply(vec![me(), refuser(2)], Me::Excluded);
        // when / then
        assert!(!admission_refused(&reply));
    }

    #[test]
    fn pending_me_is_not_a_refusal() {
        let reply = reply(vec![me(), refuser(2)], Me::Pending);
        assert!(!admission_refused(&reply));
    }
}

mod who_is_online {
    use super::*;

    #[test]
    fn nobody_else_is_not_a_refusal() {
        // given: only my own row
        let reply = reply(vec![me()], Me::Listed);
        // when / then
        assert!(!admission_refused(&reply));
    }

    #[test]
    fn someone_online_is_not_a_refusal() {
        // given: a refuser offline and another member online
        let reply = reply(vec![me(), refuser(2), other(3, true)], Me::Listed);
        assert!(!admission_refused(&reply));
    }

    #[test]
    fn an_online_refuser_is_not_a_refusal() {
        let online_refuser = MemberRow {
            online: true,
            ..refuser(2)
        };
        let reply = reply(vec![me(), online_refuser], Me::Listed);
        assert!(!admission_refused(&reply));
    }

    #[test]
    fn a_silent_offline_member_is_not_a_refusal() {
        // given: the only other member is offline and never refused me
        let reply = reply(vec![me(), other(2, false)], Me::Listed);
        assert!(!admission_refused(&reply));
    }
}

mod whose_and_which_refusal {
    use super::*;

    #[test]
    fn one_offline_member_refusing_me_is_a_refusal() {
        // given: my row is online, the only other member is offline and refused me
        let reply = reply(vec![me(), refuser(2)], Me::Listed);
        // when / then: my own online row does not count as "someone online"
        assert!(admission_refused(&reply));
    }

    #[test]
    fn my_own_refusal_does_not_count() {
        // given: the offline member is the one I refused
        let mine = MemberRow {
            last_refusal: Some(refusal("me", "excluded")),
            ..other(2, false)
        };
        let reply = reply(vec![me(), mine], Me::Listed);
        assert!(!admission_refused(&reply));
    }

    #[test]
    fn unlisted_does_not_count() {
        let reply = reply(vec![me(), refuser_for(2, "unlisted")], Me::Listed);
        assert!(!admission_refused(&reply));
    }

    #[test]
    fn other_reasons_count() {
        for reason in ["refused", "left", "excluded"] {
            let reply = reply(vec![me(), refuser_for(2, reason)], Me::Listed);
            assert!(admission_refused(&reply), "reason {reason}");
        }
    }

    #[test]
    fn several_refusers_are_a_refusal() {
        let reply = reply(vec![me(), refuser(2), refuser(3)], Me::Listed);
        assert!(admission_refused(&reply));
    }

    #[test]
    fn one_counting_refuser_is_enough() {
        // given: one refusal that does not count and one that does
        let reply = reply(
            vec![me(), refuser_for(2, "unlisted"), refuser_for(3, "excluded")],
            Me::Listed,
        );
        assert!(admission_refused(&reply));
    }
}

/// My status in the list.
enum Me {
    Listed,
    Excluded,
    Pending,
}

/// My own row: `is_self`, always online.
fn me() -> MemberRow {
    MemberRow {
        is_self: true,
        ..other(1, true)
    }
}

/// An offline member that refused me with `excluded`.
fn refuser(seed: u8) -> MemberRow {
    refuser_for(seed, "excluded")
}

fn refuser_for(seed: u8, reason: &'static str) -> MemberRow {
    MemberRow {
        last_refusal: Some(refusal("them", reason)),
        ..other(seed, false)
    }
}

fn refusal(side: &'static str, reason: &'static str) -> RefusalInfo {
    RefusalInfo {
        side,
        reason,
        at_unix: 1_000,
    }
}

/// A member of the list with no refusal.
fn other(seed: u8, online: bool) -> MemberRow {
    MemberRow {
        id: String::new(),
        id_full: pid(seed).to_string(),
        name: None,
        is_self: false,
        online,
        admin: false,
        status: "member",
        excluded: None,
        blocked_by_me: false,
        denied_by_me: None,
        last_seen_sec: None,
        last_refusal: None,
        admission: None,
    }
}

fn reply(rows: Vec<MemberRow>, status: Me) -> MembersReply {
    let counts = Counts {
        members: 0,
        excluded: 0,
        unlisted: 0,
        pending: 0,
    };
    MembersReply {
        me: pid(1).to_string(),
        me_excluded: matches!(status, Me::Excluded),
        me_pending: matches!(status, Me::Pending),
        admins: Vec::new(),
        me_admin: false,
        root: None,
        me_admission: None,
        counts,
        members: rows,
        changed: None,
    }
}
