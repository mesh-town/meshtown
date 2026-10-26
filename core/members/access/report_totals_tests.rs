//! The `denied_by_me` codes, refusals and totals of the member list reply (`report.rs`); fixtures are in
//! `report_tests.rs`.

use super::*;

mod denied_by_me_codes {
    use super::*;

    #[test]
    fn denied_by_me_codes() {
        // given: a is a member, b is excluded, c is blocked, d is unlisted, f is excluded and blocked
        let (a, b, c, d, e, f) = (pid(2), pid(3), pid(4), pid(5), pid(6), pid(7));
        let mut w = world(&[a, c, e], &[b, f]);
        w.verdicts.blocks.extend([c, f]);
        w.members.set_name(d, "stranger".into());
        // when / then: the codes follow the order of the check
        let codes: Vec<_> = [pid(1), a, b, c, d, e, f]
            .iter()
            .map(|p| w.row(*p).denied_by_me)
            .collect();
        assert_eq!(
            codes,
            [
                None,
                None,
                Some("excluded"),
                Some("blocked"),
                Some("unlisted"),
                None,
                Some("excluded")
            ]
        );
    }

    #[test]
    fn after_leaving_everyone_is_denied_and_my_row_is_not() {
        // given: I have left
        let mut w = world(&[pid(2)], &[]);
        w.exclude(pid(1), How::Left, vec![pid(1)]);
        // when / then
        assert_eq!(w.row(pid(2)).denied_by_me, Some("left"));
        assert_eq!(w.row(pid(1)).denied_by_me, None, "my own row");
    }

    #[test]
    fn unlisted_node_is_denied_as_unlisted() {
        // given: a node the list does not know
        let x = pid(9);
        let w = world(&[], &[]);
        w.members.set_name(x, "newcomer".into());
        // when / then
        let row = w.row(x);
        assert_eq!(
            (row.status, row.denied_by_me),
            ("unlisted", Some("unlisted"))
        );
    }
}

mod refusals {
    use super::*;

    #[test]
    fn refusal_from_them_is_reported() {
        // given: a refused me at the handshake with his block
        let a = pid(2);
        let mut w = world(&[a], &[]);
        w.refusal(a, Side::Them, Deny::Refused);
        // when
        let info = w.row(a).last_refusal.expect("a refusal");
        // then
        assert_eq!(
            (info.side, info.reason, info.at_unix),
            ("them", "refused", AT_UNIX + 100)
        );
    }

    #[test]
    fn refusal_by_me_names_my_reason() {
        let a = pid(2);
        let mut w = world(&[], &[a]);
        w.refusal(a, Side::Me, Deny::Excluded);
        let info = w.row(a).last_refusal.expect("a refusal");
        assert_eq!((info.side, info.reason), ("me", "excluded"));
    }

    #[test]
    fn refusal_of_an_unknown_node_makes_a_row() {
        // given: a refusal of a node nobody knows
        let x = pid(9);
        let mut w = world(&[], &[]);
        w.refusal(x, Side::Me, Deny::Unlisted);
        // when / then: the page shows it as unlisted, and it is counted
        let reply = w.reply();
        assert!(reply
            .members
            .iter()
            .any(|r| r.id_full == x.to_string() && r.status == "unlisted"));
        assert_eq!(reply.counts.unlisted, 1);
    }
}

mod totals {
    use super::*;

    #[test]
    fn counts_follow_the_rows() {
        // given: me and a, b, c listed, d excluded, e unlisted
        let (a, b, c, d, e) = (pid(2), pid(3), pid(4), pid(5), pid(6));
        let w = world(&[a, b, c], &[d]);
        w.members.set_name(e, "stranger".into());
        // when
        let counts = w.reply().counts;
        // then: members include me
        assert_eq!(
            (counts.members, counts.excluded, counts.unlisted),
            (4, 1, 1)
        );
    }

    #[test]
    fn changed_is_passed_through() {
        // given: a list of two
        let w = world(&[pid(2)], &[]);
        // when / then
        assert_eq!(w.reply().changed, None);
        assert_eq!(
            members_reply(&w.verdicts(), &w.members, &[], Some(false)).changed,
            Some(false)
        );
        assert_eq!(
            members_reply(&w.verdicts(), &w.members, &[], Some(true)).changed,
            Some(true)
        );
    }

    #[test]
    fn reply_names_me_by_full_id() {
        let w = world(&[], &[]);
        assert_eq!(w.reply().me, pid(1).to_string());
    }
}
