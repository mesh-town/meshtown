//! Tests for choosing an exit for a domain (`route.rs`): the pure function `choose_route` and the live list
//! `RouteCtx.chosen_exits`.

use super::*;
use crate::transport::net::{NodeSecret, PeerId};
use std::time::Duration;

/// A member made from one secret key.
fn pair(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

mod choose {
    use super::*;

    #[test]
    fn choose_with_empty_list_goes_direct() {
        // given: the exit list is empty, one node is online and holds an exit.
        let a = pair(1);
        let me = pair(9);
        let online: HashSet<PeerId> = [a].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(a, true)].into_iter().collect();

        // when
        let route = choose_route(&[], me, &online, &serving);

        // then
        assert_eq!(route, Route::Direct);
    }

    #[test]
    fn first_online_serving_exit_is_first() {
        // given: [a, b], both online and both hold an exit.
        let a = pair(1);
        let b = pair(2);
        let me = pair(9);
        let online: HashSet<PeerId> = [a, b].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(a, true), (b, true)].into_iter().collect();

        let route = choose_route(&[a, b], me, &online, &serving);

        assert_eq!(route, Route::Exits(vec![a, b]));
    }

    #[test]
    fn offline_first_falls_to_next() {
        // given: [a, b], a is offline.
        let a = pair(1);
        let b = pair(2);
        let me = pair(9);
        let online: HashSet<PeerId> = [b].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(a, true), (b, true)].into_iter().collect();

        let route = choose_route(&[a, b], me, &online, &serving);

        assert_eq!(route, Route::Exits(vec![b]));
    }

    #[test]
    fn exit_flag_off_is_skipped() {
        // given: [a, b], a is online but its exit is switched off.
        let a = pair(1);
        let b = pair(2);
        let me = pair(9);
        let online: HashSet<PeerId> = [a, b].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(a, false), (b, true)].into_iter().collect();

        let route = choose_route(&[a, b], me, &online, &serving);

        assert_eq!(route, Route::Exits(vec![b]));
    }

    #[test]
    fn exit_flag_missing_is_skipped() {
        // given: [a, b], a has no entry for the exit flag.
        let a = pair(1);
        let b = pair(2);
        let me = pair(9);
        let online: HashSet<PeerId> = [a, b].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(b, true)].into_iter().collect();

        let route = choose_route(&[a, b], me, &online, &serving);

        assert_eq!(
            route,
            Route::Exits(vec![b]),
            "a node without an entry in serving is not a candidate"
        );
    }

    #[test]
    fn self_in_list_is_skipped() {
        // given: own id in the exit list.
        let me = pair(9);
        let b = pair(2);
        let online: HashSet<PeerId> = [me, b].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(me, true), (b, true)].into_iter().collect();

        let route = choose_route(&[me, b], me, &online, &serving);
        assert_eq!(route, Route::Exits(vec![b]), "own id is skipped, b remains");

        let route_self_only = choose_route(&[me], me, &online, &serving);
        assert!(
            matches!(route_self_only, Route::Refuse(_)),
            "the only candidate is own id, there are no real candidates: {route_self_only:?}"
        );
    }

    #[test]
    fn duplicates_are_listed_once() {
        // given: a is repeated twice in the list.
        let a = pair(1);
        let b = pair(2);
        let me = pair(9);
        let online: HashSet<PeerId> = [a, b].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(a, true), (b, true)].into_iter().collect();

        let route = choose_route(&[a, a, b], me, &online, &serving);

        assert_eq!(route, Route::Exits(vec![a, b]));
    }

    #[test]
    fn none_online_is_refused() {
        // given: the only chosen exit is offline.
        let a = pair(1);
        let me = pair(9);
        let online: HashSet<PeerId> = HashSet::new();
        let serving: HashMap<PeerId, bool> = HashMap::new();

        let route = choose_route(&[a], me, &online, &serving);

        assert_eq!(route, Route::Refuse("chosen exits are offline"));
    }
}

mod route_ctx {
    use super::*;

    /// given `RouteCtx::new` with the initial list `[a]`, when `set_chosen_exits([b])`, then
    /// `chosen_exits() == [b]`.
    #[tokio::test]
    async fn set_exits_replaces_list() {
        let node = running_node().await;
        let a = pair(1);
        let b = pair(2);
        let ctx = RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![a]);

        ctx.set_chosen_exits(vec![b]);

        assert_eq!(ctx.chosen_exits(), vec![b]);
        node.shutdown().await;
    }
}

mod exit_dropped {
    use super::*;

    /// given `RouteCtx` with the list `[a]` and an already running `wait_until_exit_dropped(a)`; when
    /// `set_chosen_exits([b])` (`a` disappeared from the list); then the wait completes within ≤ 1 s.
    #[tokio::test]
    async fn exit_dropped_resolves_when_peer_removed() {
        let node = running_node().await;
        let a = pair(1);
        let b = pair(2);
        let ctx = RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![a]);

        let waiting = ctx.wait_until_exit_dropped(a);
        ctx.set_chosen_exits(vec![b]);

        assert!(
            tokio::time::timeout(Duration::from_secs(1), waiting)
                .await
                .is_ok(),
            "exit_dropped must complete as soon as a disappears from the list"
        );
        node.shutdown().await;
    }

    /// given the same wait `wait_until_exit_dropped(a)`; when `set_chosen_exits([a, b])` (`a` stays in the list,
    /// though not first); then the wait does not complete within 200 ms.
    #[tokio::test]
    async fn exit_dropped_waits_while_peer_still_listed() {
        let node = running_node().await;
        let a = pair(1);
        let b = pair(2);
        let ctx = RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![a]);

        let waiting = ctx.wait_until_exit_dropped(a);
        ctx.set_chosen_exits(vec![a, b]);

        assert!(
            tokio::time::timeout(Duration::from_millis(200), waiting)
                .await
                .is_err(),
            "a stays in the list — exit_dropped must not complete"
        );
        node.shutdown().await;
    }

    /// given `RouteCtx` with the list `[b]` (without `a`); when `wait_until_exit_dropped(a)`; then it completes at once
    /// (≤ 100 ms) — `a` was never in the list.
    #[tokio::test]
    async fn exit_dropped_immediate_when_peer_absent() {
        let node = running_node().await;
        let a = pair(1);
        let b = pair(2);
        let ctx = RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![b]);

        assert!(
            tokio::time::timeout(Duration::from_millis(100), ctx.wait_until_exit_dropped(a))
                .await
                .is_ok(),
            "a is absent from the list from the very start — exit_dropped must complete at once"
        );
        node.shutdown().await;
    }
}

/// A node on loopback (no relay or DHT, `node/config.rs::for_loopback_test`) is the simplest
/// way to get real `Members`/`DomainList` for `RouteCtx::new`: these types have no
/// lightweight test constructor, only `DomainList::open` on top of a real `Docs`.
async fn running_node() -> crate::node::RunningNode {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
}

mod candidates {
    use super::*;

    #[test]
    fn direct_has_no_candidates() {
        assert_eq!(Route::Direct.candidates(), Ok(None));
    }

    #[test]
    fn exits_give_their_candidates() {
        let list = vec![pair(1), pair(2)];
        assert_eq!(Route::Exits(list.clone()).candidates(), Ok(Some(list)));
    }

    #[test]
    fn refuse_gives_its_reason() {
        assert_eq!(
            Route::Refuse("chosen exits are offline").candidates(),
            Err("chosen exits are offline")
        );
    }
}
