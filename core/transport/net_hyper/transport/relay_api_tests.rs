//! The relay methods of `HyperTransport`: the `connect` limit with and without a candidate source, the source
//! switch, and the relay host that ends with the transport.

use super::*;
use crate::transport::net_hyper::test_cluster::cluster;
use crate::transport::net_hyper::test_direct::serving;
use crate::transport::net_hyper::test_relay::fixed_source;
use tokio::time::timeout;

const CLOSE_WITHIN: Duration = Duration::from_secs(3);

mod connect_limit {
    use super::*;

    #[test]
    fn with_a_source_covers_both_phases() {
        assert_eq!(connect_limit(true), RELAYED_CONNECT_LIMIT);
        assert_eq!(connect_limit(true), Duration::from_secs(14));
    }

    #[test]
    fn without_a_source_is_the_plain_limit() {
        assert_eq!(connect_limit(false), CONNECT_LIMIT);
        assert_eq!(connect_limit(false), Duration::from_secs(10));
    }

    #[test]
    fn with_a_source_stays_below_the_dial_timeout_of_the_node() {
        assert!(
            connect_limit(true) < Duration::from_secs(15),
            "the connect limit must end before the outer dial timeout"
        );
    }
}

mod relay_source {
    use super::*;

    #[tokio::test]
    async fn is_unset_on_a_fresh_transport() {
        // given
        let cl = cluster(3).await;
        // when
        let t = serving(&cl, 1).await;
        // then
        assert!(!t.relay_source_is_set());
    }

    #[tokio::test]
    async fn is_set_and_cleared_by_the_switch() {
        // given
        let cl = cluster(3).await;
        let t = serving(&cl, 1).await;
        // when
        t.set_relay_source(Some(fixed_source(vec![])));
        let set = t.relay_source_is_set();
        t.set_relay_source(None);
        // then
        assert!(set, "the source was not set");
        assert!(!t.relay_source_is_set(), "the source was not cleared");
    }
}

mod relay_host {
    use super::*;

    #[tokio::test]
    async fn close_ends_the_relay_host() {
        // given: a running host; this node is not public, so it starts no relay and only the stop is checked
        let cl = cluster(3).await;
        let t = serving(&cl, 1).await;
        let mut keys = t.serve_relay();
        // when
        let closed = timeout(CLOSE_WITHIN, t.close()).await;
        // then
        assert!(closed.is_ok(), "close did not finish in time");
        let ended = timeout(CLOSE_WITHIN, keys.changed()).await;
        assert!(
            matches!(ended, Ok(Err(_))),
            "the key channel stayed open: {ended:?}"
        );
        assert_eq!(*keys.borrow(), None);
    }

    #[tokio::test]
    async fn serve_relay_after_close_starts_nothing() {
        // given
        let cl = cluster(3).await;
        let t = serving(&cl, 1).await;
        t.close().await;
        // when
        let mut keys = t.serve_relay();
        // then
        assert_eq!(*keys.borrow(), None);
        let ended = timeout(CLOSE_WITHIN, keys.changed()).await;
        assert!(
            matches!(ended, Ok(Err(_))),
            "a relay host started after close: {ended:?}"
        );
    }

    #[tokio::test]
    async fn serve_relay_twice_gives_one_host() {
        // given
        let cl = cluster(3).await;
        let t = serving(&cl, 1).await;
        let _first = t.serve_relay();
        // when
        let mut second = t.serve_relay();
        t.close().await;
        // then: the second receiver follows the same host, so it ends with it
        let ended = timeout(CLOSE_WITHIN, second.changed()).await;
        assert!(
            matches!(ended, Ok(Err(_))),
            "the second receiver is not tied to the host: {ended:?}"
        );
    }
}
