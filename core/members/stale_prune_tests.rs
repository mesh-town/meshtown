//! Tests of the hourly sweep of departed members from the address cache and from the names (`prune_from`): files
//! on disk in a scratch directory, re-read with `load` after a tick.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;

use crate::transport::net::AddrHint;
use crate::transport::net::{NodeSecret, PeerId};

use super::*;
use crate::members::peer_names::PeerNames;
use crate::node::telemetry::TelemetryView;
use crate::transport::peers_cache::{entry_for, PeersCache};

const T: u64 = 1_000_000_000;
const D: u64 = 86_400;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("mesh-stale-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("cannot create the test directory");
    dir
}

struct Rig {
    dir: PathBuf,
    cache: PeersCache,
    names: PeerNames,
    prune_candidates_prev_tick: HashSet<PeerId>,
    live: LiveEvidence,
}

impl Rig {
    /// The cache and names on disk: `cached` — members and their cache write time, `named` — names.
    fn new(name: &str, cached: &[(u8, u64)], named: &[u8]) -> Rig {
        let dir = scratch(name);
        let mut cache = PeersCache::load(Some(&dir.join("c.json")), "net", T);
        for &(n, seen) in cached {
            let addr = SocketAddr::from(([127, 0, 0, 1], 10_000 + n as u16));
            let entry = entry_for(None, id(n), Some(&AddrHint::Ip(addr)), seen);
            assert!(cache.remember(entry, seen));
        }
        let mut names = PeerNames::load(Some(&dir.join("n.json")), "net");
        for &n in named {
            assert!(names.remember(id(n), "Name"));
        }
        if !cached.is_empty() {
            cache.save().unwrap();
        }
        if !named.is_empty() {
            names.save().unwrap();
        }
        let live = LiveEvidence {
            id: id(1),
            members: HashSet::new(),
            tel: TelemetryView::default(),
            now: Instant::now(),
            chosen_exits: Vec::new(),
        };
        Rig {
            dir,
            cache,
            names,
            prune_candidates_prev_tick: HashSet::new(),
            live,
        }
    }

    fn tick(
        &mut self,
        doc_seen: &[(u8, u64)],
        now_unix: u64,
    ) -> Result<Vec<(PeerId, Option<u64>)>, String> {
        let read = DocEvidence {
            check_seen_unix: HashMap::new(),
            doc_seen_unix: doc_seen.iter().map(|&(n, t)| (id(n), t)).collect(),
        };
        prune_from(
            Ok(read),
            &self.live,
            &mut self.prune_candidates_prev_tick,
            &mut self.cache,
            &mut self.names,
            now_unix,
        )
    }

    fn disk_cache(&self) -> HashSet<PeerId> {
        PeersCache::load(Some(&self.dir.join("c.json")), "net", T)
            .seen()
            .into_keys()
            .collect()
    }

    fn disk_names(&self) -> HashSet<PeerId> {
        PeerNames::load(Some(&self.dir.join("n.json")), "net")
            .ids()
            .into_iter()
            .collect()
    }
}

#[test]
fn old_peer_cut_after_second_tick_from_disk_and_names() {
    let x = id(2);
    let mut r = Rig::new("two", &[(2, T - 8 * D)], &[2]);
    assert!(r.tick(&[], T).unwrap().is_empty());
    assert!(r.disk_cache().contains(&x) && r.disk_names().contains(&x));
    let pruned = r.tick(&[], T).unwrap();
    assert_eq!(pruned.iter().map(|c| c.0).collect::<Vec<_>>(), vec![x]);
    assert!(!r.disk_cache().contains(&x));
    assert!(!r.disk_names().contains(&x));
}

#[test]
fn fresh_doc_between_ticks_saves_peer() {
    let x = id(2);
    let mut r = Rig::new("fresh", &[(2, T - 8 * D)], &[2]);
    r.tick(&[], T).unwrap();
    assert!(r.tick(&[(2, T - 60)], T).unwrap().is_empty());
    assert!(r.prune_candidates_prev_tick.is_empty());
    assert!(r.disk_cache().contains(&x) && r.disk_names().contains(&x));
}

#[test]
fn self_and_chosen_exit_kept() {
    let mut r = Rig::new(
        "keep",
        &[(1, T - 8 * D), (3, T - 8 * D), (2, T - 8 * D)],
        &[],
    );
    r.live.chosen_exits = vec![id(3)];
    r.tick(&[], T).unwrap();
    r.tick(&[], T).unwrap();
    assert_eq!(r.disk_cache(), [id(1), id(3)].into_iter().collect());
}

#[test]
fn static_cache_cut_when_clock_moves() {
    let x = id(2);
    let mut r = Rig::new("clock", &[(2, T - 6 * D)], &[]);
    r.tick(&[], T).unwrap();
    r.tick(&[], T).unwrap();
    assert!(r.disk_cache().contains(&x));
    r.tick(&[], T + 2 * D).unwrap();
    r.tick(&[], T + 2 * D).unwrap();
    assert!(!r.disk_cache().contains(&x));
}

#[test]
fn name_without_evidence_cut() {
    let mut r = Rig::new("name", &[], &[2]);
    r.tick(&[], T).unwrap();
    r.tick(&[], T).unwrap();
    assert!(r.disk_names().is_empty());
}

#[test]
fn failed_read_cuts_nothing() {
    let x = id(2);
    let mut r = Rig::new("fail", &[(2, T - 8 * D)], &[2]);
    r.prune_candidates_prev_tick.insert(x);
    let res = prune_from(
        Err("failure".to_string()),
        &r.live,
        &mut r.prune_candidates_prev_tick,
        &mut r.cache,
        &mut r.names,
        T,
    );
    assert!(res.is_err());
    assert_eq!(r.prune_candidates_prev_tick, [x].into_iter().collect());
    assert!(r.disk_cache().contains(&x) && r.disk_names().contains(&x));
}

#[test]
fn nothing_gone_writes_no_file() {
    let mut r = Rig::new("nofile", &[], &[]);
    r.tick(&[], T).unwrap();
    r.tick(&[], T).unwrap();
    assert!(!r.dir.join("c.json").exists());
    assert!(!r.dir.join("n.json").exists());
}

/// Members removed from the network (`cut_hidden`): cut from the cache and the names at once, without the
/// two-tick wait that `prune_from` applies to departed members.
mod hidden {
    use std::collections::BTreeSet;

    use super::*;

    mod removed_member {
        use super::*;

        #[test]
        fn forgotten_cut_on_first_tick() {
            // given: a cache and names with members 2 and 3, member 2 is forgotten
            let mut r = Rig::new("hid-forgotten", &[(2, T), (3, T)], &[2, 3]);
            // when: the first sweep runs
            let cut = r.cut(&[2]);
            // then: it is gone from memory and from disk, the other stays
            assert_eq!(cut, ids(&[2]));
            assert!(!r.cache.seen().contains_key(&id(2)));
            assert_eq!(r.disk_cache(), ids(&[3]));
            assert_eq!(r.disk_names(), ids(&[3]));
        }

        #[test]
        fn my_waiting_round_cut_on_first_tick() {
            // given: the subject of my own pending removal vote is hidden, so it is in the set too
            let mut r = Rig::new("hid-mine", &[(2, T), (3, T)], &[2, 3]);
            // when: the first sweep runs, nothing was ever seen as a candidate
            let cut = r.cut(&[2]);
            // then: the subject is cut although it is fresh and has no tick history
            assert_eq!(cut, ids(&[2]));
            assert!(r.prune_candidates_prev_tick.is_empty());
            assert_eq!(r.disk_cache(), ids(&[3]));
        }

        #[test]
        fn only_in_names_is_cut_and_reported() {
            let mut r = Rig::new("hid-name", &[(3, T)], &[2, 3]);
            let cut = r.cut(&[2]);
            assert_eq!(
                cut,
                ids(&[2]),
                "an id present in the names alone is reported"
            );
            assert_eq!(r.disk_names(), ids(&[3]));
            assert_eq!(r.disk_cache(), ids(&[3]));
        }

        #[test]
        fn only_in_cache_is_cut_and_reported() {
            let mut r = Rig::new("hid-cache", &[(2, T), (3, T)], &[3]);
            let cut = r.cut(&[2]);
            assert_eq!(
                cut,
                ids(&[2]),
                "an id present in the cache alone is reported"
            );
            assert_eq!(r.disk_cache(), ids(&[3]));
            assert_eq!(r.disk_names(), ids(&[3]));
        }
    }

    mod other_members {
        use super::*;

        #[test]
        fn others_waiting_round_not_cut() {
            // given: member 2 has someone else's pending removal round, so it is not in my hidden set
            let mut r = Rig::new("hid-others", &[(2, T), (3, T)], &[2, 3]);
            // when: the sweep cuts the hidden set (only member 3 is forgotten) and a regular first tick runs
            let cut = r.cut(&[3]);
            assert!(r.tick(&[(2, T - 60)], T).unwrap().is_empty());
            // then: member 2 stays in the cache and in the names
            assert_eq!(cut, ids(&[3]));
            assert_eq!(r.disk_cache(), ids(&[2]));
            assert_eq!(r.disk_names(), ids(&[2]));
        }

        #[test]
        fn empty_hidden_cuts_nothing_and_writes_no_file() {
            let mut r = Rig::new("hid-empty", &[], &[]);
            assert!(r.cut(&[]).is_empty());
            assert!(!r.dir.join("c.json").exists());
            assert!(!r.dir.join("n.json").exists());
        }

        #[test]
        fn hidden_unknown_to_both_is_not_reported() {
            let mut r = Rig::new("hid-unknown", &[(3, T)], &[3]);
            let cut = r.cut(&[7]);
            assert!(cut.is_empty(), "nothing to cut, nothing to report: {cut:?}");
            assert_eq!(r.disk_cache(), ids(&[3]));
            assert_eq!(r.disk_names(), ids(&[3]));
        }

        #[test]
        fn fresh_members_outside_hidden_survive_the_cut() {
            let mut r = Rig::new("hid-fresh", &[(2, T), (3, T), (4, T)], &[2, 3, 4]);
            r.cut(&[2, 4]);
            assert_eq!(r.disk_cache(), ids(&[3]));
            assert_eq!(r.disk_names(), ids(&[3]));
        }
    }

    impl Rig {
        /// `cut_hidden` over the given members, as the sweep tick does before `prune_from`.
        fn cut(&mut self, hidden: &[u8]) -> HashSet<PeerId> {
            let hidden: BTreeSet<PeerId> = hidden.iter().map(|&n| id(n)).collect();
            cut_hidden(&hidden, &mut self.cache, &mut self.names)
        }
    }

    fn ids(ns: &[u8]) -> HashSet<PeerId> {
        ns.iter().map(|&n| id(n)).collect()
    }
}
