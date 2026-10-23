//! Tests of the one-time promotion of own witnesses to votes (`boot.rs`, `boot_plan::promotions`).
//! Child of `boot_tests.rs`: `Rig`, `setup`, `NOW` and the other helpers are there.

use super::*;
use std::collections::BTreeSet;

use crate::members::access::boot_plan::promotions;
use crate::members::access::record::{
    encode, exclude_key, tally_input, withdraw, BOOT_NOTE_ADMISSION,
};
use crate::members::access::tally::tally;

/// A marker as the first stage wrote it: no flag.
async fn given_old_marker(rig: &Rig) {
    rig.given_marker(Via::Migration, NOW - DAY).await;
}

async fn flagged(rig: &Rig) -> bool {
    rig.marker()
        .await
        .is_some_and(|m| m.note == BOOT_NOTE_ADMISSION)
}

mod timing {
    use super::*;

    #[tokio::test]
    async fn old_timer_promotes_its_witnesses_once() {
        // given: a node of the first stage with a marker and witnesses for x and y
        let rig = setup();
        let (x, y) = (pid(2), pid(3));
        given_old_marker(&rig).await;
        rig.given_own_admit(x, Via::Witness).await;
        rig.given_own_admit(y, Via::Witness).await;
        // when: the first boot of this version
        rig.when_boot(&[], &[], NOW).await;
        // then: both are votes, the marker carries the flag, its origin and time are as they were
        assert_eq!(
            rig.admits().await,
            admits_of(&[(x, Via::None), (y, Via::None)])
        );
        let marker = rig.marker().await.expect("the marker stays");
        assert_eq!(
            (marker.note.as_str(), marker.via, marker.made_unix),
            (BOOT_NOTE_ADMISSION, Via::Migration, NOW - DAY)
        );
    }

    #[tokio::test]
    async fn second_boot_does_not_promote_again() {
        // given: a promoted node that wrote a new witness for z afterwards
        let rig = setup();
        let z = pid(4);
        given_old_marker(&rig).await;
        rig.when_boot(&[], &[], NOW).await;
        rig.given_own_admit(z, Via::Witness).await;
        // when: the next boot
        rig.when_boot(&[], &[], NOW + 60).await;
        // then: z stays a witness
        assert_eq!(rig.admits().await, admits_of(&[(z, Via::Witness)]));
    }

    #[tokio::test]
    async fn newcomer_promotes_nothing() {
        // given: a node with no earlier life and a neighbour from the invitation
        let rig = setup();
        let x = pid(2);
        // when: the first boot
        rig.when_boot(&[x], &[], NOW).await;
        // then: the witness stays a witness; the marker is flagged from the start
        assert_eq!(rig.admits().await, admits_of(&[(x, Via::Witness)]));
        let marker = rig.marker().await.expect("the marker is written");
        assert_eq!(
            (marker.via, marker.note.as_str()),
            (Via::None, BOOT_NOTE_ADMISSION)
        );
    }
}

mod refusals {
    use super::*;

    #[tokio::test]
    async fn withdrawn_admit_is_not_promoted_after_unadmit() {
        // given: an old marker, a withdrawn admit of x that is still a cached neighbour, a witness for y
        let rig = setup();
        let (x, y) = (pid(2), pid(3));
        given_old_marker(&rig).await;
        rig.given_own_admit(x, Via::Invite).await;
        withdraw(&*rig.shared, AclKind::Admit, x).await.unwrap();
        rig.given_own_admit(y, Via::Witness).await;
        // when: the boot
        rig.when_boot(&[], &[x], NOW).await;
        // then: x has no live record, y is a vote
        assert_eq!(rig.admits().await, admits_of(&[(y, Via::None)]));
    }

    #[tokio::test]
    async fn excluded_and_blocked_are_not_promoted() {
        // given: witnesses for an excluded node e and a blocked node b
        let rig = setup();
        let (v1, v2, e, b) = (pid(2), pid(3), pid(4), pid(5));
        given_old_marker(&rig).await;
        rig.given_own_admit(v1, Via::Invite).await;
        rig.given_own_admit(v2, Via::Invite).await;
        rig.given_own_admit(e, Via::Witness).await;
        rig.given_own_admit(b, Via::Witness).await;
        given_vote(&rig, v1, e, 10);
        given_vote(&rig, v2, e, 20);
        rig.given_block(b).await;
        // when: the boot
        rig.when_boot(&[], &[], NOW).await;
        // then: both stay witnesses; the flag is set
        let want = admits_of(&[
            (v1, Via::Invite),
            (v2, Via::Invite),
            (e, Via::Witness),
            (b, Via::Witness),
        ]);
        assert_eq!(rig.admits().await, want);
        assert!(flagged(&rig).await);
    }
}

fn given_vote(rig: &Rig, voter: PeerId, subject: PeerId, ts_micros: u64) {
    let value = AclValue {
        kind: AclKind::Exclude,
        subject,
        via: Via::None,
        made_unix: 111,
        note: String::new(),
    };
    let value = Bytes::from(encode(&value));
    rig.shared.put_raw(ReplicaEntry {
        author: voter,
        key: exclude_key(&subject),
        value,
        ts_micros,
    });
}

type Row = (PeerId, PeerId, Via);

fn snap_of(rows: &[Row]) -> Vec<ReplicaEntry> {
    let entry = |&(author, subject, via): &Row| {
        let value = AclValue {
            kind: AclKind::Admit,
            subject,
            via,
            made_unix: 111,
            note: String::new(),
        };
        ReplicaEntry {
            author,
            key: admit_key(&subject),
            value: Bytes::from(encode(&value)),
            ts_micros: 1_000,
        }
    };
    rows.iter().map(entry).collect()
}

/// The rows after each of `authors` has promoted its witnesses.
fn promoted(rows: &[Row], authors: &[PeerId]) -> Vec<Row> {
    let snap = snap_of(rows);
    let mut out = rows.to_vec();
    for me in authors {
        let ledger = tally(&tally_input(*me, &snap).0);
        let chosen = promotions(&snap, *me, &BTreeSet::new(), &ledger, &[]);
        out.iter_mut()
            .filter(|r| r.0 == *me && chosen.contains(&r.1))
            .for_each(|r| r.2 = Via::None);
    }
    out
}

fn ledger_at(me: PeerId, rows: &[Row]) -> crate::members::access::tally::Ledger {
    tally(&tally_input(me, &snap_of(rows)).0)
}

/// The network of the first stage: m invited a, a invited c, and everybody witnesses everybody.
fn first_stage() -> [Row; 6] {
    let (m, a, c) = (pid(1), pid(2), pid(3));
    [
        (m, a, Via::Invite),
        (a, m, Via::Witness),
        (a, c, Via::Invite),
        (c, a, Via::Witness),
        (c, m, Via::Witness),
        (m, c, Via::Witness),
    ]
}

mod network {
    use super::*;

    #[test]
    fn founder_and_old_timers_admit_a_newcomer_view_without_clicks() {
        // given: the first-stage network, a newcomer d with m and a in its string, invited by m
        let (m, a, c, d) = (pid(1), pid(2), pid(3), pid(9));
        let newcomer = [
            (d, m, Via::Witness),
            (d, a, Via::Witness),
            (m, d, Via::Invite),
        ];
        // when: tallied at d, with and without the promotion of the old timers
        let after = [&promoted(&first_stage(), &[m, a, c])[..], &newcomer].concat();
        let before = [&first_stage()[..], &newcomer].concat();
        let (with, without) = (ledger_at(d, &after), ledger_at(d, &before));
        // then: a and c are members at d without a click, and the single vote of a admits c either way
        assert!(with.is_member(&a) && with.is_member(&c));
        assert!(without.is_member(&c) && without.admissions.is_empty());
    }

    #[test]
    fn newcomer_with_one_member_in_the_string_sees_the_chain() {
        // given: the same, but d knows only m
        let (m, a, c, d) = (pid(1), pid(2), pid(3), pid(9));
        let newcomer = [(d, m, Via::Witness), (m, d, Via::Invite)];
        let rows = [&promoted(&first_stage(), &[m, a, c])[..], &newcomer].concat();
        // when: tallied at d
        let ledger = ledger_at(d, &rows);
        // then: a and c are members, each admitted by the single vote of the previous one
        assert!(ledger.is_member(&a) && ledger.is_member(&c));
        assert!(ledger.admissions.is_empty());
    }

    #[test]
    fn newcomer_with_the_whole_star_in_the_string_sees_it_without_clicks() {
        // given: a invited b and c, the other witnesses are promoted; n holds a, b and c in its string, invited by a
        let (a, b, c, n) = (pid(1), pid(2), pid(3), pid(9));
        let star = [
            (a, b, Via::Invite),
            (a, c, Via::Invite),
            (b, a, Via::None),
            (c, a, Via::None),
            (b, c, Via::None),
            (c, b, Via::None),
        ];
        let newcomer = [
            (n, a, Via::Witness),
            (n, b, Via::Witness),
            (n, c, Via::Witness),
            (a, n, Via::Invite),
        ];
        // when: tallied at n
        let ledger = ledger_at(n, &[&star[..], &newcomer].concat());
        // then: b and c are members without a click; the vote of a admits n itself
        assert!(ledger.is_member(&b) && ledger.is_member(&c) && ledger.is_member(&n));
        assert!(ledger.admissions.is_empty());
    }
}
