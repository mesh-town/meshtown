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
        w.exclude(pid(1), How::Left, pid(1));
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
        // given: c was excluded by a
        let (a, b, c) = (pid(2), pid(3), pid(4));
        let mut w = world(&[a, b], &[c]);
        w.exclude(c, How::Votes, a);
        // when
        let row = w.row(c);
        // then: the exclusion carries the contract fields
        let info = row.excluded.expect("exclusion info");
        assert_eq!((info.how, info.at_unix), ("removed", AT_UNIX));
        assert_eq!(info.by, a.to_string());
        assert_eq!(row.status, "excluded");
    }

    #[test]
    fn excluded_info_keys_are_the_contract() {
        // given: c was excluded by the administrator a
        let (a, c) = (pid(2), pid(4));
        let mut w = world(&[a], &[c]);
        w.exclude(c, How::Votes, a);
        w.ledger.admins.insert(pid(1));
        // when
        let json = serde_json::to_value(w.row(c).excluded.expect("exclusion info")).unwrap();
        // then: exactly how, at_unix, by and reason (I am an administrator); `by` is the full id of the author
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["at_unix", "by", "how", "reason"]);
        assert_eq!(json["by"], a.to_string());
        assert_eq!(a.to_string().len(), 64);
    }

    /// The row of `c`, excluded by `a` with `reason`, as an administrator (`me`) or an ordinary member sees it.
    fn excluded_json(admin: bool, how: How, reason: Option<&str>) -> serde_json::Value {
        let (a, c) = (pid(2), pid(4));
        let mut w = world(&[a], &[c]);
        w.exclude(c, how, a);
        w.ledger.excluded.get_mut(&c).unwrap().reason = reason.map(str::to_string);
        if admin {
            w.ledger.admins.insert(pid(1));
        }
        serde_json::to_value(w.row(c).excluded.expect("exclusion info")).unwrap()
    }

    #[test]
    fn removed_row_shows_reason_and_null_without_it() {
        // given / when: an administrator looks at a removal with a note and one without
        let with = excluded_json(true, How::Votes, Some("spam"));
        let without = excluded_json(true, How::Votes, None);
        // then: the note, or `null` with the key present
        assert_eq!(with["reason"], "spam");
        assert!(
            without.as_object().unwrap().contains_key("reason"),
            "{without}"
        );
        assert!(without["reason"].is_null(), "{without}");
    }

    #[test]
    fn non_admin_gets_no_reason_key() {
        // given / when: an ordinary member looks at a removal with a note
        let json = excluded_json(false, How::Votes, Some("spam"));
        // then: there is no key at all, not `null`
        assert!(!json.as_object().unwrap().contains_key("reason"), "{json}");
    }

    #[test]
    fn excluded_self_gets_own_reason() {
        // given: I am not an administrator and was removed by a with a note; another excluded node has one too
        let (a, other) = (pid(2), pid(4));
        let mut w = world(&[a], &[other]);
        w.ledger.admitted.insert(pid(1));
        w.exclude(pid(1), How::Votes, a);
        w.ledger.excluded.get_mut(&pid(1)).unwrap().reason = Some("spam".into());
        w.ledger.excluded.get_mut(&other).unwrap().reason = Some("rude".into());
        // when
        let mine = serde_json::to_value(w.row(pid(1)).excluded.unwrap()).unwrap();
        let theirs = serde_json::to_value(w.row(other).excluded.unwrap()).unwrap();
        // then: my own row has the note, the other row has no key
        assert_eq!(mine["reason"], "spam");
        assert!(
            !theirs.as_object().unwrap().contains_key("reason"),
            "{theirs}"
        );
    }

    #[test]
    fn left_node_row_has_null_reason() {
        // given / when: an administrator looks at a node that left
        let json = excluded_json(true, How::Left, None);
        // then: the key is there and is `null`
        assert!(json["reason"].is_null(), "{json}");
        assert!(json.as_object().unwrap().contains_key("reason"), "{json}");
    }

    #[test]
    fn left_node_is_excluded_by_himself() {
        // given: a has left
        let a = pid(2);
        let mut w = world(&[a], &[]);
        w.exclude(a, How::Left, a);
        // when / then
        let info = w.row(a).excluded.expect("exclusion info");
        assert_eq!((info.how, info.by), ("left", a.to_string()));
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
