//! Row order and the fields of the rows of the member list reply (`report.rs`); fixtures are in
//! `report_tests.rs`.

use super::*;

mod order {
    use super::*;

    #[test]
    fn self_row_first_then_members_by_name() {
        // given: me, two named members, a nameless member, an unlisted named node and an excluded one
        let (me, zeta, alpha, nameless, ghost, gone) =
            (pid(1), pid(2), pid(3), pid(4), pid(5), pid(6));
        let w = world(&[zeta, alpha, nameless], &[gone]);
        w.members.set_name(zeta, "zeta".into());
        w.members.set_name(alpha, "alpha".into());
        w.members.set_name(ghost, "ghost".into());
        w.members.set_name(me, "laptop".into());
        // when: the reply is built
        let reply = w.reply();
        // then: me, members by name (nameless last), unlisted, excluded
        let ids: Vec<&str> = reply.members.iter().map(|r| r.id_full.as_str()).collect();
        let want: Vec<String> = [me, alpha, zeta, nameless, ghost, gone]
            .iter()
            .map(|p| p.to_string())
            .collect();
        assert_eq!(ids, want.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(
            reply.members.iter().map(|r| r.status).collect::<Vec<_>>(),
            ["member", "member", "member", "member", "unlisted", "excluded"]
        );
    }

    #[test]
    fn nameless_rows_of_one_kind_go_by_id() {
        // given: two nameless members
        let (b, c) = (pid(3), pid(2));
        let w = world(&[b, c], &[]);
        // when / then: after my row they are ordered by the full id
        let ids: Vec<String> = w
            .reply()
            .members
            .iter()
            .skip(1)
            .map(|r| r.id_full.clone())
            .collect();
        let mut want = vec![b.to_string(), c.to_string()];
        want.sort();
        assert_eq!(ids, want);
    }

    #[test]
    fn my_row_is_first_even_when_excluded() {
        // given: I have left; a is listed
        let a = pid(2);
        let mut w = world(&[a], &[]);
        w.exclude(pid(1), How::Left, vec![pid(1)]);
        // when / then: my row is the first one and the reply says so
        let reply = w.reply();
        assert!(reply.me_excluded);
        assert!(reply.members[0].is_self, "my row must come first");
        assert_eq!(reply.members[0].status, "excluded");
    }
}

mod fields {
    use super::*;

    #[test]
    fn excluded_row_fields_match_contract() {
        // given: c was excluded by the votes of a and b
        let (a, b, c) = (pid(2), pid(3), pid(4));
        let mut w = world(&[a, b], &[c]);
        w.exclude(c, How::Votes, vec![a, b]);
        // when
        let row = w.row(c);
        // then: the exclusion carries the contract fields
        let info = row.excluded.expect("exclusion info");
        assert_eq!((info.how, info.at_unix), ("removed", AT_UNIX));
        assert_eq!(info.voters, vec![a.to_string(), b.to_string()]);
        assert_eq!(row.status, "excluded");
    }

    #[test]
    fn left_node_is_excluded_with_himself_as_voter() {
        // given: a has left
        let a = pid(2);
        let mut w = world(&[a], &[]);
        w.exclude(a, How::Left, vec![a]);
        // when / then
        let info = w.row(a).excluded.expect("exclusion info");
        assert_eq!((info.how, info.voters), ("left", vec![a.to_string()]));
    }

    #[test]
    fn online_follows_connections_and_my_row_is_online() {
        // given: a is connected, b is not
        let (a, b) = (pid(2), pid(3));
        let w = world(&[a, b], &[]);
        w.members.add_verified_conn(a);
        // when / then
        assert!(w.row(pid(1)).online, "my row");
        assert!(w.row(a).online, "connected");
        assert!(!w.row(b).online, "not connected");
    }

    #[test]
    fn blocked_node_not_on_the_list_shows_as_unlisted() {
        // given: I blocked a node that was never listed
        let x = pid(9);
        let mut w = world(&[], &[]);
        w.verdicts.blocks.insert(x);
        // when
        let row = w.row(x);
        // then
        assert_eq!((row.status, row.blocked_by_me), ("unlisted", true));
    }

    #[test]
    fn id_is_short_and_id_full_is_the_whole_key() {
        let w = world(&[pid(2)], &[]);
        let row = w.row(pid(2));
        assert_eq!(row.id_full, pid(2).to_string());
        assert!(
            row.id_full.starts_with(&row.id) && row.id.len() < row.id_full.len(),
            "short id: {}",
            row.id
        );
    }
}

mod own_name {
    use super::*;

    #[test]
    fn self_row_carries_own_name() {
        // given: the own node has no name among the announced ones
        let w = world(&[], &[]);
        // when: the reply is built with the name the node runs under
        let reply = members_reply(&w.verdicts(), &w.members, "alpha", &w.refusals, None);
        // then: the own row has it
        let own = reply.members.iter().find(|r| r.is_self).expect("own row");
        assert_eq!(own.name.as_deref(), Some("alpha"));
    }

    #[test]
    fn self_row_without_name_is_null() {
        // given: no name anywhere
        let w = world(&[], &[]);
        // when: the node name is empty
        let reply = members_reply(&w.verdicts(), &w.members, "", &w.refusals, None);
        // then: the own row has none
        let own = reply.members.iter().find(|r| r.is_self).expect("own row");
        assert_eq!(own.name, None);
    }
}
