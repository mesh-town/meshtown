//! Tests of the boot of the membership list (`boot.rs`), second part: whom a boot must never admit, what a
//! restart re-asserts, the ceiling on new records and the text of the log line. Child of `boot_tests.rs`:
//! `Rig`, `setup`, `NOW` and the other helpers are there.

use super::*;
use std::collections::BTreeSet;

use crate::access::record::{block_key, encode, exclude_key, withdraw};
use crate::net::NodeSecret;

mod restart {
    use super::*;

    #[tokio::test]
    async fn restart_reasserts_cached_admits() {
        // given: a node that booted once, with an admit of X made by an invitation
        let rig = setup();
        let (x, newcomer) = (pid(2), pid(3));
        rig.given_own_admit(x, Via::Invite).await;
        rig.when_boot(&[], &[x], NOW).await;
        let stored = rig.shared.get(rig.me, &admit_key(&x)).await.unwrap();
        // when: it starts again, and the cache now holds a new neighbour too
        let out = rig.when_boot(&[], &[x, newcomer], NOW + 3600).await;
        // then: the new one is written, the old record stays as it was (same time, still `Invite`)
        assert_eq!(rig.admits().await, admits_of(&[(x, Via::Invite), (newcomer, Via::Witness)]));
        assert_eq!(rig.shared.get(rig.me, &admit_key(&x)).await.unwrap(), stored, "the record of X was rewritten");
        assert_eq!(out, 1);
    }

    #[tokio::test]
    async fn boot_skips_my_withdrawn_admit() {
        // given: an admit of X that the node has withdrawn (a tombstone lies under the key)
        let rig = setup();
        let x = pid(2);
        rig.given_own_admit(x, Via::Invite).await;
        rig.when_boot(&[], &[], NOW).await;
        withdraw(&*rig.shared, AclKind::Admit, x).await.unwrap();
        // when: it starts again with X in the cache
        let out = rig.when_boot(&[], &[x], NOW + 3600).await;
        // then: the withdrawn vote is not written again: X stays out
        assert_eq!(rig.admits().await, admits_of(&[]));
        assert_eq!(out, 0);
    }
}

mod never_admits {
    use super::*;

    #[tokio::test]
    async fn never_admits_excluded_voted_or_blocked() {
        // given: V1 and V2 are admitted by me; E is excluded by their two votes, W carries my own vote,
        // B is blocked on this node; all of them are known neighbours, together with a clean node N
        let rig = setup();
        let (v1, v2, e, w, b, n) = (pid(2), pid(3), pid(4), pid(5), pid(6), pid(7));
        rig.given_own_admit(v1, Via::Invite).await;
        rig.given_own_admit(v2, Via::Invite).await;
        rig.given_admit_by(v1, e);
        rig.given_admit_by(v1, w);
        rig.given_vote_by(v1, e, 10);
        rig.given_vote_by(v2, e, 20);
        rig.given_own_vote(w).await;
        rig.given_block(b).await;
        // when: the boot runs with all of them (and the node itself) as neighbours
        let out = rig.when_boot(&[e, w, b, rig.me], &[n], NOW).await;
        // then: only the clean node is written
        let want = admits_of(&[(v1, Via::Invite), (v2, Via::Invite), (n, Via::Witness)]);
        assert_eq!(rig.admits().await, want);
        assert_eq!(out, 1);
    }

    #[tokio::test]
    async fn withdrawn_vote_of_mine_does_not_stop_the_admit() {
        // given: a vote of mine against X that I withdrew
        let rig = setup();
        let x = pid(2);
        rig.given_own_vote(x).await;
        withdraw(&*rig.shared, AclKind::Exclude, x).await.unwrap();
        // when: the boot runs with X as a neighbour
        let out = rig.when_boot(&[x], &[], NOW).await;
        // then: X is admitted
        assert_eq!(rig.admits().await, admits_of(&[(x, Via::Witness)]));
        assert_eq!(out, 1);
    }

    #[tokio::test]
    async fn exclusion_by_not_yet_admitted_voters_is_respected() {
        // given: a long-time node; V1 and V2 are recent authors (they will be admitted only by the migration)
        // and have voted against S, who is a cached neighbour
        let rig = setup();
        let (v1, v2, s) = (pid(2), pid(3), pid(4));
        rig.given_prior_life().await;
        rig.given_vote_by(v1, s, ago(2 * DAY));
        rig.given_vote_by(v2, s, ago(DAY));
        // when: the first boot
        rig.when_boot(&[], &[s], NOW).await;
        // then: the voters are admitted and their votes count: S is excluded, so S is not admitted
        let want = admits_of(&[(v1, Via::Migration), (v2, Via::Migration)]);
        assert_eq!(rig.admits().await, want, "S must stay out");
    }

    #[tokio::test]
    async fn left_node_admits_nothing() {
        // given: a node with a prior life that has excluded itself, with neighbours and a recent author around
        let rig = setup();
        let (initial, cached, author) = (pid(2), pid(3), pid(4));
        rig.given_prior_life().await;
        rig.given_own_vote(rig.me).await;
        rig.given_record_of(author, b"node/exit/a", ago(DAY));
        // when: the first boot
        let out = rig.when_boot(&[initial], &[cached], NOW).await;
        // then: no admits, the marker is written all the same
        assert!(rig.admits().await.is_empty(), "a node that left admits nobody");
        assert_eq!(out, 0);
        assert!(rig.marker().await.is_some(), "the marker must be written");
    }
}

mod ceiling {
    use super::*;

    #[tokio::test]
    async fn admit_cap_prefers_witness_then_recent() {
        // given: a long-time node with 100 cached neighbours and 200 recent authors (300 candidates)
        let rig = setup();
        let (cached, authors) = (many_ids(1, 100), many_ids(2, 200));
        rig.given_prior_life().await;
        rig.given_authors_by_age(&authors);
        // when: the first boot
        let out = rig.when_boot(&[], &cached, NOW).await;
        // then: 256 records: every witness, then the 156 newest authors; the 44 oldest are left for the next start
        let written = rig.admits().await;
        assert_eq!(written.len(), 256);
        assert_eq!(out, 256);
        assert!(cached.iter().all(|c| written.get(c) == Some(&Via::None)), "every witness must be written and promoted");
        assert!(authors[44..].iter().all(|a| written.get(a) == Some(&Via::Migration)), "the newest 156 authors");
        assert!(authors[..44].iter().all(|a| !written.contains_key(a)), "the oldest 44 authors");
    }

    #[tokio::test]
    async fn admit_cap_counts_skipped_candidates() {
        // given: the same 300 candidates, planned without writing anything
        let rig = setup();
        let (cached, authors) = (many_ids(1, 100), many_ids(2, 200));
        rig.given_authors_by_age(&authors);
        let snap = rig.shared.scan(b"").await.unwrap();
        let boot = AccessBoot { me: rig.me, initial: vec![], cached: cached.clone(), now_unix: NOW };
        // when: the plan is made with the migration on
        let plan = plan_admits(&snap, &BTreeSet::new(), &boot, Some(NOW));
        // then: the witnesses first, 156 authors, 44 skipped, and the order is newest first
        assert_eq!(plan.witness, cached);
        assert_eq!(plan.migration.len(), 156);
        assert_eq!(plan.skipped, 44);
        let newest_first: Vec<PeerId> = authors[44..].iter().rev().copied().collect();
        assert_eq!(plan.migration, newest_first, "authors by their last record, newest first");
    }

    #[tokio::test]
    async fn restart_does_not_write_what_the_ceiling_left() {
        // given: the first boot has written 256 of 300 candidates
        let rig = setup();
        let (cached, authors) = (many_ids(1, 100), many_ids(2, 200));
        rig.given_prior_life().await;
        rig.given_authors_by_age(&authors);
        rig.when_boot(&[], &cached, NOW).await;
        // when: the node starts again
        let out = rig.when_boot(&[], &cached, NOW + 60).await;
        // then: the marker is there, so the migration is not repeated and the 44 oldest authors stay out
        assert_eq!(out, 0);
        assert_eq!(rig.admits().await.len(), 256);
    }
}

mod log_line {
    use super::*;

    #[test]
    fn boot_line_is_ascii_without_ids() {
        // given: plans that carry real node ids, for every class of a boot
        let ids = [pid(2), pid(3), pid(4)];
        let plan = plan_of(&ids[..2], &ids[2..], 7, 3);
        let left = BootPlan { me_left: true, ..plan_of(&[], &[], 0, 0) };
        // when: the lines are made
        let lines = [
            boot_line(Class::Legacy, &plan, 3),
            boot_line(Class::Fresh, &plan, 2),
            boot_line(Class::Done, &plan, 1),
            boot_line(Class::Done, &plan, 0),
            boot_line(Class::Left, &left, 0),
        ];
        // then: ASCII, no id in any spelling, no long run of hex letters and digits
        for line in &lines {
            assert!(line.is_ascii(), "not ASCII: {line}");
            assert!(line.starts_with("acl boot: "), "the prefix the manual run looks for: {line}");
            assert!(ids.iter().all(|id| !line.contains(&id.to_string())), "full id in: {line}");
            assert!(ids.iter().all(|id| !line.contains(&id.fmt_short().to_string())), "short id in: {line}");
            assert!(!has_hex_word(line), "a hex-looking word in: {line}");
        }
    }

    #[test]
    fn boot_line_follows_the_documented_texts() {
        // given: a plan with two witnesses and one migrated author
        let ids = [pid(2), pid(3), pid(4)];
        let plan = plan_of(&ids[..2], &ids[2..], 0, 0);
        // when / then: the first start of a long-time node, the repeats, the node that left
        assert_eq!(
            boot_line(Class::Legacy, &plan, 3),
            "acl boot: legacy, admitted 3 (witness 2, migration 1)"
        );
        assert_eq!(boot_line(Class::Fresh, &plan, 2), "acl boot: fresh, admitted 2");
        assert_eq!(boot_line(Class::Done, &plan, 1), "acl boot: done earlier, admitted 1");
        assert_eq!(boot_line(Class::Done, &plan, 0), "acl boot: done earlier, admitted 0");
        let left = BootPlan { me_left: true, ..plan_of(&[], &[], 0, 0) };
        assert_eq!(boot_line(Class::Left, &left, 0), "acl boot: left the network, nothing admitted");
    }

    #[test]
    fn boot_line_adds_skipped_and_ignored_only_when_not_zero() {
        // given: plans that skipped some candidates and met unreadable list records
        let plan = plan_of(&[pid(2)], &[], 44, 3);
        let only_skipped = plan_of(&[pid(2)], &[], 44, 0);
        // when / then: the tail names the counts, in this order, and is absent for zero
        assert_eq!(boot_line(Class::Fresh, &plan, 1), "acl boot: fresh, admitted 1, skipped 44, ignored 3");
        assert_eq!(boot_line(Class::Fresh, &only_skipped, 1), "acl boot: fresh, admitted 1, skipped 44");
    }
}

// --- helpers ---

impl Rig {
    /// An admit record written by another author.
    fn given_admit_by(&self, author: PeerId, subject: PeerId) {
        let value = AclValue { kind: AclKind::Admit, subject, via: Via::Invite, made_unix: 111, note: String::new() };
        self.given_list_record(author, admit_key(&subject), &value, 1_000);
    }

    /// An exclusion vote written by another author at the given time.
    fn given_vote_by(&self, voter: PeerId, subject: PeerId, ts_micros: u64) {
        let value = AclValue { kind: AclKind::Exclude, subject, via: Via::None, made_unix: 111, note: String::new() };
        self.given_list_record(voter, exclude_key(&subject), &value, ts_micros);
    }

    fn given_list_record(&self, author: PeerId, key: Vec<u8>, value: &AclValue, ts_micros: u64) {
        let value = Bytes::from(encode(value));
        self.shared.put_raw(ReplicaEntry { author, key, value, ts_micros });
    }

    /// My own vote to exclude the subject (the subject may be the node itself: leaving).
    async fn given_own_vote(&self, subject: PeerId) {
        let value = AclValue { kind: AclKind::Exclude, subject, via: Via::None, made_unix: 111, note: String::new() };
        put(&*self.shared, &value).await.unwrap();
    }

    /// A block of the subject in the own (local) replica.
    pub(super) async fn given_block(&self, subject: PeerId) {
        let value = AclValue { kind: AclKind::Block, subject, via: Via::None, made_unix: 111, note: String::new() };
        put(&*self.own, &value).await.unwrap();
        assert!(self.own.get(self.me, &block_key(&subject)).await.unwrap().is_some(), "the block must be stored");
    }

    /// One record per author; the later in the list, the newer (an hour apart, all within 9 days).
    fn given_authors_by_age(&self, authors: &[PeerId]) {
        let count = authors.len() as u64;
        for (i, author) in authors.iter().enumerate() {
            self.given_record_of(*author, b"node/exit/a", ago((count - i as u64) * 3600));
        }
    }
}

/// `n` distinct node ids; `tag` separates the families.
fn many_ids(tag: u8, n: usize) -> Vec<PeerId> {
    let id = |i: usize| {
        let mut seed = [tag; 32];
        seed[..8].copy_from_slice(&(i as u64).to_be_bytes());
        NodeSecret::from_bytes(&seed).peer_id()
    };
    (0..n).map(id).collect()
}

fn plan_of(witness: &[PeerId], migration: &[PeerId], skipped: usize, ignored: usize) -> BootPlan {
    BootPlan { witness: witness.to_vec(), migration: migration.to_vec(), skipped, ignored, me_left: false, promote: Vec::new() }
}

/// A run of 8 or more hex characters that contains a letter: the look of an id (a plain number is no id).
fn has_hex_word(line: &str) -> bool {
    let is_hex = |c: char| c.is_ascii_hexdigit() && !c.is_ascii_uppercase();
    line.split(|c: char| !is_hex(c)).any(|run| run.len() >= 8 && run.chars().any(|c| c.is_ascii_alphabetic()))
}
