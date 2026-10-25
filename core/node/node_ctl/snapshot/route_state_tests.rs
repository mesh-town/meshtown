//! Tests of `route_state`: the `route::choose_route` decision into the snapshot's `route` object. They do not check
//! `route::choose_route` itself (that is `route_tests.rs`) — only the transfer of the decision and the candidate list
//! into `RouteState`/`ChosenExit`.
//!
//! `Inputs` here is built by the `inputs()` helper below (without the `blocked` field, with the
//! `chosen_exits` field). If the real `Inputs` in `build.rs` changes its set of fields, fix only the literal
//! in `inputs()` and leave the test bodies alone.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::transport::net::PeerAddr;
use crate::transport::net::{NodeSecret, PeerId};

use super::*;
use crate::node::output;
use crate::node::telemetry::TelemetryView;
use crate::web_gui::{ChosenExit, PeerInfo, RouteMode};

/// A member made from one secret key — as in `route_tests.rs`.
fn pair(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

/// `Inputs` with neutral values for all fields except those that decide path selection.
fn inputs(id: PeerId, online: HashSet<PeerId>, serving: HashMap<PeerId, bool>) -> Inputs<'static> {
    Inputs {
        id,
        me: id,
        self_name: None,
        members: online,
        neighbors: 0,
        votes: Default::default(),
        socks: None,
        socks_access: (
            crate::config::config_file::ListenAccess::Localhost,
            Vec::new(),
        ),
        socks_ifaces: Default::default(),
        exit_on: false,
        exit_clients: Vec::new(),
        exit_rates: HashMap::new(),
        exits: serving,
        countries: HashMap::new(),
        program_versions: HashMap::new(),
        peer_names: HashMap::new(),
        chosen_exits: Vec::new(),
        excluded: HashSet::new(),
        removed: HashSet::new(),
        domain_names: Vec::new(),
        check_views: Vec::new(),
        in_progress: HashSet::new(),
        telemetry: TelemetryView::default(),
        addr: PeerAddr::new(id),
        now_unix: 1_000,
        now_instant: Instant::now(),
        cached_seen: HashMap::new(),
        doc_seen: HashMap::new(),
    }
}

/// A `peers[]` row for node `id` — as `build_peers` builds it (id/id_full/online/exit).
fn peer_info(id: PeerId, online: bool, exit: Option<bool>) -> PeerInfo {
    PeerInfo {
        id: output::short_peer_id(&id),
        id_full: id.to_string(),
        name: None,
        is_self: false,
        online,
        path: None,
        rtt_ms: None,
        exit,
        country: None,
        program_version: None,
        checked: 0,
        stale: false,
        last_seen_sec: 0,
        through_peer_bps: 0,
        through_us_bps: 0,
    }
}

fn active_exit(exits: &[ChosenExit]) -> Option<&ChosenExit> {
    exits.iter().find(|e| e.active)
}

mod when_chosen_list_is_empty {
    use super::*;

    #[test]
    fn state_is_direct_with_no_exits() {
        // given: the exit list is empty.
        let my_id = pair(1);
        let i = inputs(my_id, HashSet::new(), HashMap::new());

        // when
        let state = route_state(&[], &i, &[]);

        // then
        assert_eq!(state.state, RouteMode::Direct);
        assert!(state.exits.is_empty());
    }
}

mod when_first_candidate_is_online_and_serving {
    use super::*;

    #[test]
    fn state_is_exit_with_that_candidate_active() {
        // given: [a] is online and serves the exit.
        let my_id = pair(1);
        let a = pair(2);
        let online: HashSet<PeerId> = [a].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(a, true)].into_iter().collect();
        let i = inputs(my_id, online, serving);
        let peers = vec![peer_info(a, true, Some(true))];

        let state = route_state(&[a], &i, &peers);

        assert_eq!(state.state, RouteMode::Exit);
        assert_eq!(state.exits.len(), 1);
        assert!(
            state.exits[0].active,
            "the only candidate is also the active one"
        );
        assert_eq!(state.exits[0].id_full, a.to_string());
    }
}

mod when_first_candidate_is_offline_and_second_is_online {
    use super::*;

    #[test]
    fn only_second_is_active_and_first_reported_offline() {
        // given: [a, b], a is offline, b is online and serves the exit.
        let my_id = pair(1);
        let a = pair(2);
        let b = pair(3);
        let online: HashSet<PeerId> = [b].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(b, true)].into_iter().collect();
        let i = inputs(my_id, online, serving);
        let peers = vec![peer_info(a, false, None), peer_info(b, true, Some(true))];

        let state = route_state(&[a, b], &i, &peers);

        assert_eq!(state.state, RouteMode::Exit);
        assert_eq!(
            state.exits.len(),
            2,
            "both candidates of the list are in exits, not only the active one"
        );
        let exit_a = state
            .exits
            .iter()
            .find(|e| e.id_full == a.to_string())
            .expect("a stays in the list even though it is offline");
        let exit_b = state
            .exits
            .iter()
            .find(|e| e.id_full == b.to_string())
            .expect("b is the active exit");
        assert!(!exit_a.online, "a.online = false");
        assert!(!exit_a.active, "only b is active");
        assert!(exit_b.active);
        assert_eq!(
            active_exit(&state.exits).map(|e| &e.id_full),
            Some(&b.to_string())
        );
    }
}

mod when_only_candidate_is_offline {
    use super::*;

    #[test]
    fn state_is_unavailable_with_no_active_exit() {
        // given: [a], a is offline — the only candidate is unavailable.
        let my_id = pair(1);
        let a = pair(2);
        let i = inputs(my_id, HashSet::new(), HashMap::new());
        let peers = vec![peer_info(a, false, None)];

        let state = route_state(&[a], &i, &peers);

        assert_eq!(state.state, RouteMode::Unavailable);
        assert!(active_exit(&state.exits).is_none(), "no exit is active");
    }
}

// The config allows the node's own id in the list of chosen exits, but `route::choose_route`
// skips it as a candidate — so the single entry gives `state = Unavailable`.
// `route_state` still finds the node's own `peers[]` row by `id_full` (not by the short
// `id`) to return its online/exit in `exits[0]`, even though the exit is not active.
mod when_chosen_list_contains_only_self {
    use super::*;

    #[test]
    fn own_peer_is_found_by_id_full_and_listed_inactive() {
        // given: the own node is the only entry of route.exits; it is also in peers[].
        let my_id = pair(1);
        let i = inputs(my_id, HashSet::new(), HashMap::new());
        let self_peer = PeerInfo {
            is_self: true,
            ..peer_info(my_id, true, Some(true))
        };

        let state = route_state(&[my_id], &i, &[self_peer]);

        assert_eq!(
            state.state,
            RouteMode::Unavailable,
            "the own id is not a candidate"
        );
        assert_eq!(state.exits.len(), 1);
        assert_eq!(
            state.exits[0].id_full.parse::<PeerId>().unwrap(),
            my_id,
            "the own row's id_full parses back into the own PeerId"
        );
        assert!(!state.exits[0].active);
    }
}

mod removed {
    use super::*;

    /// `Inputs` where `x` is online and serving, and `excluded` holds the given nodes.
    fn setup(me: PeerId, x: PeerId, excluded: &[PeerId]) -> Inputs<'static> {
        let online: HashSet<PeerId> = [x].into_iter().collect();
        let serving: HashMap<PeerId, bool> = [(x, true)].into_iter().collect();
        let mut i = inputs(me, online, serving);
        i.excluded = excluded.iter().copied().collect();
        i
    }

    mod exit_with_an_exclusion_record {
        use super::*;

        #[test]
        fn is_marked_removed_and_keeps_its_online_state() {
            // given: x is a chosen exit, online, and the list holds an exclusion record for it
            let (me, x) = (pair(1), pair(2));
            let i = setup(me, x, &[x]);
            let peers = vec![peer_info(x, true, Some(true))];

            // when
            let state = route_state(&[x], &i, &peers);

            // then
            assert!(state.exits[0].removed, "an excluded exit is removed");
            assert!(state.exits[0].online, "online is computed as before");
        }
    }

    mod exit_without_a_record {
        use super::*;

        #[test]
        fn is_not_removed() {
            // given: x is a chosen exit, another node is excluded
            let (me, x) = (pair(1), pair(2));
            let i = setup(me, x, &[pair(3)]);
            let peers = vec![peer_info(x, true, Some(true))];

            // when
            let state = route_state(&[x], &i, &peers);

            // then
            assert!(!state.exits[0].removed);
        }
    }

    mod own_node {
        use super::*;

        #[test]
        fn is_never_removed() {
            // given: the own id is a chosen exit and is also in the excluded set
            let me = pair(1);
            let i = setup(me, pair(2), &[me]);
            let self_peer = PeerInfo {
                is_self: true,
                ..peer_info(me, true, Some(true))
            };

            // when
            let state = route_state(&[me], &i, &[self_peer]);

            // then
            assert!(
                !state.exits[0].removed,
                "the own node is not marked removed"
            );
        }
    }

    mod json {
        use super::*;

        #[test]
        fn not_removed_has_no_removed_key() {
            // given: a default exit
            let exit = ChosenExit::default();

            // when
            let json = serde_json::to_value(&exit).unwrap();

            // then
            assert!(
                json.get("removed").is_none(),
                "a false flag is not serialized"
            );
        }

        #[test]
        fn removed_is_serialized_as_true() {
            // given: a removed exit
            let exit = ChosenExit {
                removed: true,
                ..ChosenExit::default()
            };

            // when
            let json = serde_json::to_value(&exit).unwrap();

            // then
            assert_eq!(json["removed"], serde_json::json!(true));
        }
    }
}
