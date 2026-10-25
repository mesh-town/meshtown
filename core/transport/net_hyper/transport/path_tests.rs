//! What a session knows about its path: round-trip time on both sides, and the address that goes to the cache.

use crate::transport::net::addr::PeerAddr;
use crate::transport::net::contract::transport_cases::{echo_mismatches, spawn_echo};
use crate::transport::net::contract::STEP;
use crate::transport::net::transport::Transport;
use crate::transport::net_hyper::test_cluster::cluster;
use crate::transport::net_hyper::test_direct::{
    becomes, connect_within, direct_addr, ip_of, isolated, serving,
};
use crate::transport::peers_cache::{entry_for, PeersCache};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

const FIVE_SECONDS: Duration = Duration::from_secs(5);
const NETWORK: &str = "test-network";
const NOW: u64 = 1_000_000;

mod rtt {
    use super::*;

    #[tokio::test]
    async fn is_known_after_an_exchange_on_both_sides() {
        // given
        let cl = cluster(3).await;
        let (s, n) = (serving(&cl, 1).await, serving(&cl, 2).await);
        let ours = connect_within(&n, &PeerAddr::new(s.me()), Duration::from_secs(15))
            .await
            .expect("connection by key");
        let (_, theirs) = timeout(STEP, s.accept())
            .await
            .expect("accept deadline")
            .expect("incoming");
        let _echo = spawn_echo(theirs.clone());
        // when
        let bad = echo_mismatches(&ours, 1, 64 << 10, Duration::from_secs(15)).await;
        // then
        assert!(bad.is_empty(), "echo did not match");
        assert!(
            becomes(FIVE_SECONDS, || ours.link().rtt.is_some()).await,
            "no rtt at the dialing side"
        );
        assert!(
            becomes(FIVE_SECONDS, || theirs.link().rtt.is_some()).await,
            "no rtt at the accepting side"
        );
    }

    #[tokio::test]
    async fn path_text_of_a_live_session_has_the_rtt() {
        // given
        let cl = cluster(3).await;
        let (s, n) = (serving(&cl, 1).await, serving(&cl, 2).await);
        let ours = connect_within(&n, &PeerAddr::new(s.me()), Duration::from_secs(15))
            .await
            .expect("connection by key");
        let (_, theirs) = timeout(STEP, s.accept())
            .await
            .expect("accept deadline")
            .expect("incoming");
        let _echo = spawn_echo(theirs.clone());
        // when
        let bad = echo_mismatches(&ours, 1, 64 << 10, Duration::from_secs(15)).await;
        // then
        assert!(bad.is_empty(), "echo did not match");
        assert!(
            becomes(FIVE_SECONDS, || ours.link().rtt.is_some()).await,
            "no rtt at the dialing side"
        );
        let text = ours.path_text();
        assert!(text.contains(" ms") && !text.contains("unknown"), "{text}");
    }
}

mod accepted {
    use super::*;

    #[tokio::test]
    async fn reports_no_remote_and_no_cache_entry() {
        // given: N dials S directly, S accepts
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let n = isolated(2).await;
        let _ours = connect_within(&n, &direct_addr(&s).await, FIVE_SECONDS)
            .await
            .expect("direct connection");
        let (_, theirs) = timeout(STEP, s.accept())
            .await
            .expect("accept deadline")
            .expect("incoming");
        // when
        let hint = theirs.link().hint();
        // then
        assert!(
            hint.is_none(),
            "the source of a handshake is not an address: {hint:?}"
        );
        let mut cache = PeersCache::load(None, NETWORK, NOW);
        let entry = entry_for(None, n.me(), hint.as_ref(), NOW);
        assert!(
            !cache.remember(entry, NOW),
            "nothing to remember about the dialer"
        );
    }
}

mod cache {
    use super::*;

    #[tokio::test]
    async fn proven_address_survives_restart_with_dead_dht() {
        // given: a direct session recorded in the cache file, then the node goes away
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let file = scratch("restart").join("peers-cache.json");
        let first = isolated(2).await;
        let conn = connect_within(&first, &direct_addr(&s).await, FIVE_SECONDS)
            .await
            .expect("direct connection");
        let mut cache = PeersCache::load(Some(&file), NETWORK, NOW);
        let entry = entry_for(None, s.me(), conn.link().hint().as_ref(), NOW);
        assert!(
            cache.remember(entry, NOW),
            "the direct address must be cached"
        );
        cache.save().expect("cache saved");
        first.close().await;
        // when: the restarted node reads the cache and still has a dead DHT
        let second = isolated(2).await;
        for addr in PeersCache::load(Some(&file), NETWORK, NOW).addrs() {
            second.add_hints(&addr);
        }
        let res = connect_within(&second, &PeerAddr::new(s.me()), FIVE_SECONDS).await;
        // then
        assert!(res.is_ok(), "the cached address was not dialed");
    }

    #[tokio::test]
    async fn entry_keeps_the_dialed_address() {
        // given
        let cl = cluster(3).await;
        let s = serving(&cl, 1).await;
        let n = isolated(2).await;
        let invite = direct_addr(&s).await;
        // when
        let conn = connect_within(&n, &invite, FIVE_SECONDS)
            .await
            .expect("direct connection");
        // then
        let entry = entry_for(None, s.me(), conn.link().hint().as_ref(), NOW);
        assert_eq!(entry.addrs, vec![ip_of(&invite)]);
    }
}

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "mesh-hyper-path-{name}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("could not create the test's temporary directory");
    dir
}
