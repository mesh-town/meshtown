use super::*;
use crate::node::output::{log_page, short_peer_id};
use crate::transport::net::facade::test_support::{addr, pid, within};
use std::time::Duration;
use tokio::sync::{Mutex, MutexGuard};

/// The log is shared per process: line tests run one at a time.
pub(in crate::node) static LOG_LOCK: Mutex<()> = Mutex::const_new(());

async fn lock() -> MutexGuard<'static, ()> {
    LOG_LOCK.lock().await
}

fn last_seq() -> u64 {
    log_page(0, 1).last_seq
}

/// Texts of log lines with tag `tag` that appeared after `after`.
fn lines(tag: &str, after: u64) -> Vec<String> {
    let page = log_page(after, 10_000);
    page.lines
        .into_iter()
        .filter(|l| l.tag == tag)
        .map(|l| l.text)
        .collect()
}

fn cache(list: Vec<PeerAddr>, me: PeerId) -> Arc<dyn Discovery> {
    Arc::new(PeersCacheSource::new(list, me))
}

fn invite(list: Vec<PeerAddr>, me: PeerId) -> Arc<dyn Discovery> {
    Arc::new(InviteSource::new(list, me))
}

async fn take(rx: &mut mpsc::Receiver<PeerAddr>) -> PeerAddr {
    within("receiving an address", rx.recv())
        .await
        .expect("address")
}

#[tokio::test]
async fn cache_hands_list_in_order() {
    let _g = lock().await;
    let after = last_seq();
    let (tx, mut rx) = mpsc::channel(8);
    let task = tokio::spawn(
        cache(vec![addr(11, &[]), addr(12, &[]), addr(13, &[])], pid(1)).run_source(tx),
    );
    for n in [11, 12, 13] {
        assert_eq!(take(&mut rx).await.id, pid(n));
    }
    drop(rx);
    within("end of run", task).await.unwrap();
    assert_eq!(
        lines("cache", after),
        ["peer addresses taken from cache: 3"]
    );
}

#[tokio::test]
async fn own_id_skipped() {
    let _g = lock().await;
    let after = last_seq();
    let (tx, mut rx) = mpsc::channel(8);
    let task = tokio::spawn(cache(vec![addr(1, &[]), addr(21, &[])], pid(1)).run_source(tx));
    assert_eq!(take(&mut rx).await.id, pid(21));
    drop(rx);
    within("end of run", task).await.unwrap();
    assert_eq!(
        lines("cache", after),
        ["peer addresses taken from cache: 1"]
    );

    let after = last_seq();
    let (tx, mut rx) = mpsc::channel(8);
    let task = tokio::spawn(invite(vec![addr(1, &[]), addr(22, &[])], pid(1)).run_source(tx));
    assert_eq!(take(&mut rx).await.id, pid(22));
    drop(rx);
    within("end of run", task).await.unwrap();
    assert_eq!(lines("invite", after), ["addresses taken from invite: 1"]);
}

#[tokio::test]
async fn full_out_logs_not_accepted() {
    let _g = lock().await;
    let after = last_seq();
    let (tx, rx) = mpsc::channel(1);
    let task = tokio::spawn(invite(vec![addr(31, &[]), addr(32, &[])], pid(1)).run_source(tx));
    tokio::time::sleep(Duration::from_millis(200)).await;
    let got = lines("invite", after);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(
        got[0].starts_with(&format!(
            "address {} not accepted: ",
            short_peer_id(&pid(32))
        )),
        "{got:?}"
    );
    assert_eq!(got[1], "addresses taken from invite: 1");
    drop(rx);
    within("end of run", task).await.unwrap();
}

#[tokio::test]
async fn run_waits_until_out_closed() {
    let _g = lock().await;
    let (tx, mut rx) = mpsc::channel(8);
    let mut task = tokio::spawn(cache(vec![addr(41, &[])], pid(1)).run_source(tx));
    assert_eq!(take(&mut rx).await.id, pid(41));
    assert!(tokio::time::timeout(Duration::from_millis(300), &mut task)
        .await
        .is_err());
    drop(rx);
    within("end of run", task).await.unwrap();
}

#[tokio::test]
async fn empty_list_says_nothing() {
    let _g = lock().await;
    let after = last_seq();
    let (tx, rx) = mpsc::channel(8);
    let (tx2, rx2) = mpsc::channel(8);
    let mut a = tokio::spawn(cache(vec![], pid(1)).run_source(tx));
    let mut b = tokio::spawn(invite(vec![], pid(1)).run_source(tx2));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(lines("cache", after).is_empty());
    assert!(lines("invite", after).is_empty());
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut a)
        .await
        .is_err());
    assert!(tokio::time::timeout(Duration::from_millis(100), &mut b)
        .await
        .is_err());
    drop((rx, rx2));
    within("end of cache", a).await.unwrap();
    within("end of invite", b).await.unwrap();
}

#[tokio::test]
async fn closed_out_returns_at_once() {
    let _g = lock().await;
    let after = last_seq();
    let (tx, rx) = mpsc::channel(8);
    drop(rx);
    within("run", cache(vec![addr(51, &[])], pid(1)).run_source(tx)).await;
    let got = lines("cache", after);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(
        got[0].starts_with(&format!(
            "address {} not accepted: ",
            short_peer_id(&pid(51))
        )),
        "{got:?}"
    );
    assert_eq!(got[1], "peer addresses taken from cache: 0");
}

#[tokio::test]
async fn names_and_publish() {
    let c = cache(vec![], pid(1));
    let i = invite(vec![], pid(1));
    assert_eq!((c.name(), i.name()), ("cache", "invite"));
    assert!(!c.publishes() && !i.publishes());
    within("publish", c.publish(&addr(1, &[]))).await.unwrap();
    within("publish", i.publish(&addr(1, &[]))).await.unwrap();
}

/// The node's source list from the config (6b): order `invite`, `cache`.
mod node_list {
    use super::*;
    use crate::members::network_key::NetworkKey;
    use crate::node::config::NodeConfig;
    use crate::node::telemetry::Telemetry;
    use crate::transport::net_mainline::record::RosterEntry;
    use crate::transport::peers_cache::PeersCache;

    fn filled_cache() -> PeersCache {
        let mut cache = PeersCache::load(None, "net", 1000);
        let entry = RosterEntry {
            id: *pid(61).as_bytes(),
            relay: None,
            addrs: vec!["127.0.0.1:4000".parse().unwrap()],
            seen_unix: 1000,
        };
        assert!(cache.remember(entry, 1000));
        cache
    }

    fn names(cfg: &NodeConfig, cache: &PeersCache, subkeys: &SubKeys) -> Vec<&'static str> {
        node_sources(cfg, cache, pid(1), subkeys, &Telemetry::new(), None)
            .iter()
            .map(|d| d.name())
            .collect()
    }

    /// A node config with a non-empty invite and everything `node_sources` needs; the discovery order is the build default.
    struct Env {
        cfg: NodeConfig,
        subkeys: SubKeys,
    }

    impl Env {
        /// Names of the node's sources by the current config and the `cache` cache.
        fn names(&self, cache: &PeersCache) -> Vec<&'static str> {
            names(&self.cfg, cache, &self.subkeys)
        }
    }

    async fn setup() -> Env {
        let key = NetworkKey::generate().unwrap();
        let subkeys = key.subkeys();
        let mut cfg = NodeConfig::for_loopback_test(key);
        cfg.initial_peers = vec![addr(62, &[])];
        Env { cfg, subkeys }
    }

    fn order(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| n.to_string()).collect()
    }

    #[tokio::test]
    async fn node_sources_follow_config() {
        let key = NetworkKey::generate().unwrap();
        let subkeys = key.subkeys();
        let base = NodeConfig::for_loopback_test(key);
        let empty = PeersCache::load(None, "net", 1000);
        assert!(names(&base, &empty, &subkeys).is_empty());

        let mut invited = base.clone();
        invited.initial_peers = vec![addr(62, &[])];
        assert_eq!(names(&invited, &empty, &subkeys), ["invite"]);
        assert_eq!(names(&base, &filled_cache(), &subkeys), ["cache"]);

        let mut all = invited;
        all.settings.net_choice.discovery = order(&["invite", "cache"]);
        assert_eq!(names(&all, &filled_cache(), &subkeys), ["invite", "cache"]);
    }

    #[tokio::test]
    async fn sources_follow_configured_order() {
        // given: the invite and the cache are non-empty, the discovery order in the config is cache, then invite
        let mut env = setup().await;
        env.cfg.settings.net_choice.discovery = order(&["cache", "invite"]);
        // when: the node builds the sources
        let got = env.names(&filled_cache());
        // then: the sources go in the config order, not in the default order
        assert_eq!(got, ["cache", "invite"]);
    }

    #[tokio::test]
    async fn dropped_name_has_no_source() {
        // given: the cache is non-empty, but only "invite" is left in the config's discovery list
        let mut env = setup().await;
        env.cfg.settings.net_choice.discovery = order(&["invite"]);
        // when: the node builds the sources
        let got = env.names(&filled_cache());
        // then: there is no "cache" source, only "invite" is left
        assert_eq!(
            got,
            ["invite"],
            "a source outside the config list must not be built"
        );
    }

    #[tokio::test]
    async fn hyperdht_without_records_is_skipped() {
        // given: "hyperdht" is in the discovery list, there are no hyper transport records
        let mut env = setup().await;
        env.cfg.settings.net_choice.discovery = order(&["invite", "hyperdht"]);
        // when: the node builds the sources
        let got = env.names(&filled_cache());
        // then: "hyperdht" is skipped, the rest are in place
        assert_eq!(got, ["invite"]);
    }

    #[tokio::test]
    async fn mainline_name_ignored() {
        // given: the name "mainline" remained in an old setting
        let mut env = setup().await;
        env.cfg.settings.net_choice.discovery = order(&["invite", "cache", "mainline"]);
        // then: no source, the rest are in place
        assert_eq!(env.names(&filled_cache()), ["invite", "cache"]);
    }

    #[test]
    fn seen_handler_fills_telemetry_and_logs_on_change() {
        // given: a handler over fresh telemetry
        let tel = Telemetry::new();
        let h = seen_handler(tel.clone());
        // when: a record with 3 members is read and published, then 3 again and then 4
        h(DhtRoundOutcome {
            read: true,
            members: 3,
            published: true,
        });
        let v = tel.view();
        // then: read, publish and join are marked
        assert_eq!(v.dht_read.map(|r| r.1), Some(3));
        assert!(v.dht_joined && v.last_publish.is_some() && v.record_published);
        h(DhtRoundOutcome {
            read: true,
            members: 3,
            published: true,
        });
        h(DhtRoundOutcome {
            read: true,
            members: 4,
            published: true,
        });
        let lines = crate::node::output::log_page(0, 10_000).lines;
        let n = |m: &str| {
            lines
                .iter()
                .filter(|l| l.tag == "hyperdht" && l.text.contains(m))
                .count()
        };
        assert_eq!(n("peers 3"), 1, "a repeat of the same N isn't written");
        assert_eq!(n("peers 4"), 1);
    }

    #[test]
    fn seen_handler_unread_leaves_no_read_time() {
        let tel = Telemetry::new();
        seen_handler(tel.clone())(DhtRoundOutcome {
            read: false,
            members: 0,
            published: false,
        });
        let v = tel.view();
        assert!(v.dht_read.is_none() && !v.dht_joined);
    }
}
