//! Tests of the overload fields of the snapshot: when a record counts, the own row, the chosen exit and `exit.load`.

use super::tests::{base, pair};
use super::*;
use crate::exit::load::Reason;
use crate::exit::load::{Cause, ExitLoad, MemberLoad};
use crate::exit::{load::LoadSnapshot, LoadView};
use crate::web_gui::NodeSnapshot;

const MIN: u64 = 60 * 1_000_000;

/// Inputs at `now_unix = 100_000` with member `b` online and flagged as an exit, and its record `age_min` old
/// (negative — stamped in the future).
fn with_record(age_min: i64) -> (Inputs<'static>, PeerId) {
    let (me, b) = (pair(1), pair(2));
    let mut i = base(me);
    i.now_unix = 100_000;
    i.members = [b].into_iter().collect();
    i.exits = [(b, true)].into_iter().collect();
    let ts = (i.now_unix * 1_000_000) as i64 - age_min * MIN as i64;
    let cause = Cause::Known(Reason::Share);
    let record = ExitLoad {
        cause,
        ts_micros: ts as u64,
    };
    i.load.records.insert(b, record);
    (i, b)
}

fn row_of(i: &Inputs<'_>, id: PeerId) -> crate::web_gui::PeerInfo {
    let full = id.to_string();
    build_peers(i)
        .into_iter()
        .find(|p| p.id_full == full)
        .expect("row")
}

#[test]
fn peer_overloaded_needs_online_exit_and_fresh_record() {
    // given: a fresh record of an online exit
    let (i, b) = with_record(24);
    assert_eq!(
        row_of(&i, b).overloaded.as_deref(),
        Some("share"),
        "24 minutes ago"
    );
    let (i, b) = with_record(-24);
    assert_eq!(
        row_of(&i, b).overloaded.as_deref(),
        Some("share"),
        "24 minutes ahead"
    );
    // then: each broken condition gives null
    let (i, b) = with_record(26);
    assert_eq!(row_of(&i, b).overloaded, None, "26 minutes ago");
    let (i, b) = with_record(-26);
    assert_eq!(row_of(&i, b).overloaded, None, "26 minutes ahead");
    let (mut i, b) = with_record(1);
    i.members.clear();
    i.cached_seen.insert(b, 99_990);
    assert_eq!(row_of(&i, b).overloaded, None, "not online");
    let (mut i, b) = with_record(1);
    i.exits.insert(b, false);
    assert_eq!(row_of(&i, b).overloaded, None, "exit flag off");
}

fn view(reason: Option<Reason>, members: Vec<MemberLoad>, refused: u64) -> LoadView {
    let counts = LoadSnapshot {
        overloaded: reason,
        admitted: 50,
        refused,
        members,
    };
    LoadView {
        streams: 3,
        limit: 100,
        limit_auto: 200,
        counts,
    }
}

#[test]
fn own_row_reads_live_state_not_document() {
    // given: the document says nothing about us, the live state says "memory"
    let mut i = base(pair(1));
    i.exit_on = true;
    i.load.own = Some(view(Some(Reason::Memory), Vec::new(), 0));
    // then: the own row shows the live reason
    assert_eq!(row_of(&i, pair(1)).overloaded.as_deref(), Some("memory"));
}

#[test]
fn own_row_is_null_when_exit_of_this_net_is_off() {
    let mut i = base(pair(1));
    i.exit_on = false;
    i.load.own = Some(view(Some(Reason::Limit), Vec::new(), 0));
    assert_eq!(row_of(&i, pair(1)).overloaded, None);
}

#[test]
fn chosen_exit_busy_is_record_or_local() {
    let (mut i, b) = with_record(1);
    i.chosen_exits = vec![b];
    let exit = |i: &Inputs<'_>| {
        route_state(&i.chosen_exits, i, &build_peers(i))
            .exits
            .remove(0)
    };
    // record and local sign
    i.load.turned_away.insert(b, (7, 20));
    let e = exit(&i);
    assert_eq!(e.overloaded.as_deref(), Some("share"));
    assert_eq!(
        e.turned_away.map(|t| (t.refused, t.attempts)),
        Some((7, 20))
    );
    assert!(e.busy);
    // record only
    i.load.turned_away.clear();
    let e = exit(&i);
    assert!(e.busy && e.turned_away.is_none());
    // local sign only
    i.load.records.clear();
    i.load.turned_away.insert(b, (5, 10));
    let e = exit(&i);
    assert!(e.busy && e.overloaded.is_none());
    // neither
    i.load.turned_away.clear();
    assert!(!exit(&i).busy);
}

#[test]
fn exit_load_is_null_when_exit_off() {
    let mut i = base(pair(1));
    i.exit_on = false;
    i.load.own = Some(view(None, Vec::new(), 0));
    assert!(build(i).snap.exit.load.is_none());
}

#[test]
fn exit_load_lists_only_members_of_this_net_and_counts_the_rest() {
    // given: 9 refusals to a member, 4 to a stranger (another network), 13 in all
    let (mine, other) = (pair(2), pair(3));
    let rows = vec![
        MemberLoad {
            peer: mine,
            admitted: 10,
            refused: 9,
        },
        MemberLoad {
            peer: other,
            admitted: 1,
            refused: 4,
        },
    ];
    let mut i = base(pair(1));
    i.exit_on = true;
    i.members = [mine].into_iter().collect();
    i.load.own = Some(view(Some(Reason::Share), rows, 13));
    // when
    let load = build(i).snap.exit.load.expect("load");
    // then
    assert_eq!((load.streams, load.limit, load.limit_auto), (3, 100, 200));
    assert_eq!(load.overloaded.as_deref(), Some("share"));
    assert_eq!(load.members.len(), 1);
    assert_eq!(load.members[0].id_full, mine.to_string());
    assert_eq!(load.elsewhere_refused, 4);
}

#[test]
fn snapshot_json_has_new_fields() {
    let (mut i, b) = with_record(1);
    i.exit_on = true;
    i.chosen_exits = vec![b];
    i.load.turned_away.insert(b, (5, 10));
    i.load.own = Some(view(None, Vec::new(), 0));
    let snap: NodeSnapshot = build(i).snap;
    let json = serde_json::to_value(&snap).expect("json");
    assert!(
        json["peers"][0]["overloaded"].is_null(),
        "own row, string or null"
    );
    assert_eq!(json["peers"][1]["overloaded"], "share");
    let exit = &json["route"]["exits"][0];
    assert_eq!(exit["overloaded"], "share");
    assert_eq!(exit["turned_away"]["refused"], 5);
    assert_eq!(exit["turned_away"]["attempts"], 10);
    assert_eq!(exit["busy"], true);
    let load = &json["exit"]["load"];
    for key in [
        "streams",
        "limit",
        "limit_auto",
        "admitted",
        "refused",
        "elsewhere_refused",
    ] {
        assert!(load[key].is_u64(), "{key}");
    }
    assert!(load["overloaded"].is_null());
    assert!(load["members"].is_array());
}
