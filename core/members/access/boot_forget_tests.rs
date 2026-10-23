//! Tests of the boot plan next to forget votes (`boot_plan.rs`): a node with a forget circle is
//! neither witnessed, nor promoted, nor migrated, because an admission written at boot would
//! return a forgotten node or clear somebody else's pending vote.
//!
//! Nodes: `me` is the own node. `a` and `b` are members admitted by `me`. `c` joined through `a`,
//! it is the subject of the forget votes. `d` and `e` are bystanders with no circle.

use std::collections::BTreeSet;

use bytes::Bytes;

use super::*;
use crate::members::access::record::{encode, key_for, AclKind, AclValue, Via};
use crate::members::access::tally::{tally, AdmitIn};
use crate::transport::net::contract::pid;
use crate::transport::net::{PeerId, ReplicaEntry};

const NOW: u64 = 1_792_400_000;
const DAY: u64 = 86_400;
const MICROS: u64 = 1_000_000;

mod witnesses {
    use super::*;

    #[test]
    fn forgotten_cached_peer_gets_no_witness() {
        // given: c is forgotten by a and b, c and a bystander d are cached neighbours
        let mut w = setup();
        w.given_forget_votes(&[w.a, w.b], w.c);
        // when: the boot plans its admissions
        let plan = w.when_plan(&[w.c, w.d], None);
        // then: only the bystander is witnessed
        assert_eq!(plan.witness, vec![w.d]);
    }

    #[test]
    fn my_forget_subject_in_refused_set() {
        // given: my own pending vote to forget c, c is a cached neighbour
        let mut w = setup();
        w.given_forget_votes(&[w.me], w.c);
        // when: the boot plans its admissions
        let plan = w.when_plan(&[w.c, w.d], None);
        // then: no witness for c, the bystander still gets one
        assert_eq!(plan.witness, vec![w.d]);
        let refused = w.refused();
        assert!(refused.contains(&w.c), "a subject of my forget vote is refused");
        assert!(!refused.contains(&w.d), "a bystander is not refused");
    }

    #[test]
    fn forgotten_subject_is_in_refused_set() {
        // given: c is forgotten by a and b
        let mut w = setup();
        w.given_forget_votes(&[w.a, w.b], w.c);
        // when: the refused set is built
        let refused = w.refused();
        // then: c is in it, a bystander is not
        assert!(refused.contains(&w.c), "a forgotten node is refused");
        assert!(!refused.contains(&w.d), "a bystander is not refused");
    }

    #[test]
    fn newcomer_witness_for_forgotten_does_not_return() {
        // given: a and b forgot c; c is the cached neighbour of this boot
        // Open point: whether the witness for c is written or dropped is not asserted; only that
        // c is no member after the witnesses of the plan are reconciled with the forget votes.
        let mut w = setup();
        w.given_forget_votes(&[w.a, w.b], w.c);
        w.snap.retain(|e| e.key != key_for(AclKind::Admit, &w.c));
        // when: the boot plans its admissions and they are tallied together with the replica
        let plan = w.when_plan(&[w.c], None);
        // then: c is not a member
        let (mut input, _) = tally_input(w.me, &w.snap);
        input.admits.extend(plan.witness.iter().map(|s| AdmitIn {
            author: w.me,
            subject: *s,
            via: Via::Witness,
            made_unix: NOW,
        }));
        assert!(!tally(&input).is_member(&w.c), "a witness cannot return a forgotten node");
    }
}

mod promotion {
    use super::*;

    #[test]
    fn forgotten_witness_is_not_promoted() {
        // given: my witnesses for c (forgotten) and for e (no circle)
        let mut w = setup();
        w.given_forget_votes(&[w.a, w.b], w.c);
        w.given_own_witness(w.c);
        w.given_own_witness(w.e);
        // when: the boot plans its admissions
        let plan = w.when_plan(&[], None);
        // then: only e becomes a vote
        assert_eq!(plan.promote, vec![w.e]);
    }

    #[test]
    fn waiting_round_witness_is_not_promoted() {
        // given: a pending vote of a against c, my witnesses for c and for e
        let mut w = setup();
        w.given_forget_votes(&[w.a], w.c);
        w.given_own_witness(w.c);
        w.given_own_witness(w.e);
        // when: the boot plans its admissions
        let plan = w.when_plan(&[], None);
        // then: c is left alone, so the vote of a stays pending; e becomes a vote
        assert_eq!(plan.promote, vec![w.e]);
    }

    #[test]
    fn promotions_skip_a_subject_with_a_circle() {
        // given: a pending vote against c and my witnesses for c and e
        let mut w = setup();
        w.given_forget_votes(&[w.a], w.c);
        w.given_own_witness(w.c);
        w.given_own_witness(w.e);
        let (input, _) = tally_input(w.me, &w.snap);
        let ledger = tally(&input);
        // when: the promotions are listed directly
        let out = promotions(&w.snap, w.me, &BTreeSet::new(), &ledger, &[]);
        // then: the same as in the plan
        assert_eq!(out, vec![w.e]);
    }
}

mod migration {
    use super::*;

    #[test]
    fn waiting_round_subject_is_not_migrated() {
        // given: c (pending vote of a against it) and d were active a day ago
        let mut w = setup();
        w.given_forget_votes(&[w.a], w.c);
        w.given_activity(w.c);
        w.given_activity(w.d);
        // when: the boot of a long-time node plans the migration
        let plan = w.when_plan(&[], Some(NOW));
        // then: d is migrated, c is not: a migration admit would clear the pending vote
        assert_eq!(plan.migration, vec![w.d]);
    }

    #[test]
    fn forgotten_recent_author_is_not_migrated() {
        // given: c is forgotten but was active a day ago, d was too
        let mut w = setup();
        w.given_forget_votes(&[w.a, w.b], w.c);
        w.given_activity(w.c);
        w.given_activity(w.d);
        // when: the boot of a long-time node plans the migration
        let plan = w.when_plan(&[], Some(NOW));
        // then: only d is migrated
        assert_eq!(plan.migration, vec![w.d]);
    }

    #[test]
    fn no_circle_means_the_author_is_migrated() {
        // given: c without any forget vote and d, both active a day ago
        let mut w = setup();
        w.given_activity(w.c);
        w.given_activity(w.d);
        // when: the boot of a long-time node plans the migration
        let plan = w.when_plan(&[], Some(NOW));
        // then: control: both are migrated (c has no admit of mine yet)
        assert_eq!(plan.migration.len(), 2);
        assert!(plan.migration.contains(&w.c) && plan.migration.contains(&w.d));
    }
}

// --- setup and helpers ---

struct World {
    me: PeerId,
    a: PeerId,
    b: PeerId,
    c: PeerId,
    d: PeerId,
    e: PeerId,
    snap: Vec<ReplicaEntry>,
}

/// `me` admitted `a` and `b`, `a` admitted `c`: four members, so two votes forget a member.
fn setup() -> World {
    let mut w = World {
        me: pid(1),
        a: pid(2),
        b: pid(3),
        c: pid(4),
        d: pid(5),
        e: pid(6),
        snap: Vec::new(),
    };
    w.put(w.me, AclKind::Admit, w.a, Via::Invite, 0);
    w.put(w.me, AclKind::Admit, w.b, Via::Invite, 0);
    w.put(w.a, AclKind::Admit, w.c, Via::Invite, 0);
    w
}

impl World {
    fn put(&mut self, author: PeerId, kind: AclKind, subject: PeerId, via: Via, made_unix: u64) {
        let value = AclValue {
            kind,
            subject,
            via,
            made_unix,
            note: String::new(),
        };
        self.snap.push(ReplicaEntry {
            author,
            key: key_for(kind, &subject),
            value: Bytes::from(encode(&value)),
            ts_micros: 0,
        });
    }

    fn given_forget_votes(&mut self, voters: &[PeerId], subject: PeerId) {
        for (i, v) in voters.iter().enumerate() {
            self.put(*v, AclKind::Forget, subject, Via::None, 100 + i as u64 * 10);
        }
    }

    fn given_own_witness(&mut self, subject: PeerId) {
        self.put(self.me, AclKind::Admit, subject, Via::Witness, 50);
    }

    /// A record of `author` outside `acl/` made a day before the boot.
    fn given_activity(&mut self, author: PeerId) {
        self.snap.push(ReplicaEntry {
            author,
            key: b"node/exit/x".to_vec(),
            value: Bytes::from_static(b"x"),
            ts_micros: (NOW - DAY) * MICROS,
        });
    }

    fn when_plan(&self, cached: &[PeerId], migrate: Option<u64>) -> BootPlan {
        let boot = AccessBoot {
            me: self.me,
            initial: Vec::new(),
            cached: cached.to_vec(),
            now_unix: NOW,
        };
        plan_admits(&self.snap, &BTreeSet::new(), &boot, migrate)
    }

    fn refused(&self) -> BTreeSet<PeerId> {
        let (input, _) = tally_input(self.me, &self.snap);
        refused_set(&self.snap, self.me, &BTreeSet::new(), &tally(&input))
    }
}
