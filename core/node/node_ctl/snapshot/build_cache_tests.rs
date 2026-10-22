//! Tests for merging the address cache (`Inputs.cached_seen`) into `peers[]`: the cache
//! fills in ids missing from the three live sources (`members`, `tel.peers`, authors of
//! domain checks) and overrides nothing in them. The wiring `collect() → Inputs.cached_seen`
//! is checked by a separate test on a live node — `collect_tests.rs`.
//! The helpers `base`/`pair`/`view`/`record_at` come from `super::tests` (`pub(super)`); this module is
//! a sibling of `mod tests` and `mod exit_tests`, not their descendant.

use super::tests::{base, pair, record_at, view};
use super::*;
use std::time::Duration;

use crate::node::telemetry::PeerLive;

mod peers_iz_kesha_zhivoe_chlenstvo_telemetriya_i_proverki_pusty {
    use super::*;

    #[test]
    fn cached_peer_appears_offline_after_restart() {
        let my_id = pair(1);
        let b = pair(2);
        let mut inputs = base(my_id);
        inputs.cached_seen = [(b, 400)].into_iter().collect();

        let collected = build(inputs);
        let peers = collected.snap.peers;
        assert_eq!(peers.len(), 2, "own row and B, known only from the cache");

        let b_peer = peers
            .iter()
            .find(|p| p.id == output::short_peer_id(&b))
            .expect("B should appear in peers from the cache");
        assert!(
            !b_peer.online,
            "a member known only from the cache is offline"
        );
        assert_eq!(
            b_peer.name, None,
            "the cache has no name, and Members does not know it either"
        );
        assert_eq!(
            b_peer.last_seen_sec, 600,
            "now_unix (1000) - cache seen_unix (400)"
        );
        assert_eq!(b_peer.path, None);
        assert_eq!(b_peer.rtt_ms, None);
        assert_eq!(b_peer.exit, None);
    }

    #[test]
    fn cached_peer_takes_known_name() {
        let my_id = pair(1);
        let b = pair(2);
        let mut inputs = base(my_id);
        inputs.cached_seen = [(b, 400)].into_iter().collect();
        inputs.peer_names = [(b, "Bob".to_string())].into_iter().collect();

        let collected = build(inputs);
        let b_peer = collected
            .snap
            .peers
            .iter()
            .find(|p| p.id == output::short_peer_id(&b))
            .expect("B should appear in peers from the cache");
        assert_eq!(b_peer.name, Some("Bob".to_string()));
    }

    #[test]
    fn cached_peers_do_not_change_members_count() {
        let my_id = pair(1);
        let b = pair(2);
        let c = pair(3);
        let mut inputs = base(my_id);
        inputs.cached_seen = [(b, 100), (c, 200)].into_iter().collect();

        let collected = build(inputs);
        assert_eq!(
            collected.snap.members.count, 0,
            "members is empty — members from the cache do not count as live membership"
        );
        assert_eq!(
            collected.snap.peers.len(),
            3,
            "own row + B and C, known only from the cache"
        );
    }

    #[test]
    fn cache_seen_in_future_gives_zero_last_seen() {
        let my_id = pair(1);
        let b = pair(2);
        let mut inputs = base(my_id);
        inputs.cached_seen = [(b, 5_000)].into_iter().collect(); // later than now_unix (1000)

        let collected = build(inputs);
        let b_peer = collected
            .snap
            .peers
            .iter()
            .find(|p| p.id == output::short_peer_id(&b))
            .expect("B should appear in peers from the cache");
        assert_eq!(
            b_peer.last_seen_sec, 0,
            "a cache record from the future must not yield a negative/overflowed value"
        );
    }
}

mod peers_iz_kesha_id_uzhe_est_u_drugogo_istochnika {
    use super::*;

    #[test]
    fn cached_peer_also_live_member_is_single_online_row() {
        let my_id = pair(1);
        let b = pair(2);
        let mut inputs = base(my_id);
        inputs.members = [b].into_iter().collect();
        inputs.cached_seen = [(b, 1)].into_iter().collect();

        let collected = build(inputs);
        let rows: Vec<_> = collected
            .snap
            .peers
            .iter()
            .filter(|p| p.id == output::short_peer_id(&b))
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "an id from the cache and from members is one row, not duplicated"
        );
        assert!(rows[0].online, "B is a live member");
        assert_eq!(
            rows[0].last_seen_sec, 0,
            "online — last_seen_sec is always 0"
        );
    }

    #[test]
    fn cached_peer_also_in_telemetry_keeps_telemetry_last_seen() {
        let my_id = pair(1);
        let b = pair(2);
        let mut inputs = base(my_id);
        let now = inputs.now_instant;
        inputs.telemetry.peers.insert(
            b,
            PeerLive {
                online: false,
                path: None,
                rtt_ms: None,
                last_seen: now - Duration::from_secs(30),
            },
        );
        inputs.cached_seen = [(b, 1)].into_iter().collect();

        let collected = build(inputs);
        let rows: Vec<_> = collected
            .snap
            .peers
            .iter()
            .filter(|p| p.id == output::short_peer_id(&b))
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "telemetry and cache are the same member, one row"
        );
        assert_eq!(
            rows[0].last_seen_sec, 30,
            "telemetry takes priority over the cache, not the cache seen_unix"
        );
    }

    #[test]
    fn cached_peer_also_domain_author_keeps_check_time() {
        let my_id = pair(1);
        let b_id = pair(2);
        let mut inputs = base(my_id);
        inputs.domain_names = vec!["a.example".to_string()];
        inputs.check_views = vec![view(
            "a.example",
            b_id,
            900,
            Some(record_at(CheckVerdict::Ok, 900)),
        )];
        inputs.cached_seen = [(b_id, 100)].into_iter().collect();

        let collected = build(inputs);
        let rows: Vec<_> = collected
            .snap
            .peers
            .iter()
            .filter(|p| p.id == output::short_peer_id(&b_id))
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "the author of a domain check and a cache record are the same member"
        );
        assert_eq!(
            rows[0].last_seen_sec, 100,
            "now_unix (1000) - check time (900) = 100, not the cache seen_unix (100 matched by chance)"
        );
        assert_eq!(rows[0].checked, 1);
    }
}

mod peers_iz_kesha_svoy_uzel {
    use super::*;

    #[test]
    fn own_id_in_cache_is_not_duplicated() {
        let my_id = pair(1);
        let mut inputs = base(my_id);
        inputs.cached_seen = [(my_id, 900)].into_iter().collect();

        let collected = build(inputs);
        assert_eq!(
            collected.snap.peers.len(),
            1,
            "the own id from the cache must not produce a second row"
        );
        assert!(collected.snap.peers[0].is_self);
    }
}

mod peers_iz_kesha_kesh_pust {
    use super::*;

    #[test]
    fn empty_cache_keeps_peers_as_before() {
        let my_id = pair(1);
        let inputs = base(my_id); // cached_seen is empty by default

        let collected = build(inputs);
        assert_eq!(collected.snap.peers.len(), 1, "own row only");
        assert!(collected.snap.peers[0].is_self);
    }
}
