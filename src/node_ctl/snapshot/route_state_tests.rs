//! Tests of `route_state`: the `route::choose_route` decision into the snapshot's `route` object. They do not check
//! `route::choose_route` itself (that is `route_tests.rs`) — only the transfer of the decision and the candidate list
//! into `RouteState`/`ChosenExit`.
//!
//! `Inputs` here is built by the `inputs()` helper below (without the `blocked` field, with the
//! `chosen_exits` field). If the real `Inputs` in `build.rs` changes its set of fields, fix only the literal
//! in `inputs()` and leave the test bodies alone.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::net::{NodeSecret, PeerId};
use crate::net::PeerAddr;

use super::*;
use crate::output;
use crate::telemetry::TelemetryView;
use crate::web::{ChosenExit, PeerInfo, RouteMode};

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
        mode: "direct_and_relay",
        members: online,
        neighbors: 0,
        votes: Default::default(),
        socks: None,
        socks_access: (crate::config_file::ListenAccess::Localhost, Vec::new()),
        socks_ifaces: Default::default(),
        exit_on: false,
        exit_clients: Vec::new(),
        exit_rates: HashMap::new(),
        exits: serving,
        countries: HashMap::new(),
        program_versions: HashMap::new(),
        peer_names: HashMap::new(),
        chosen_exits: Vec::new(),
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
        assert!(
            active_exit(&state.exits).is_none(),
            "no exit is active"
        );
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
