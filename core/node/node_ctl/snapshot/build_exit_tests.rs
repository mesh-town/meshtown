//! Test of the rule for `build_peers` (another member's snapshot row): the exit flag is taken from the map
//! of announcements `Inputs.exits` only for an online member, offline is always `None`. It does not check
//! reading the document (`node_state::read_exit_flags`) — that is `core/tests/exit_flag.rs` and `tests/node_web.rs`.
//! The helpers `base`/`pair` come from `super::tests` (`pub(super)`); this module is a sibling of `mod tests`,
//! not its descendant.

use std::time::Instant;

use super::tests::{base, pair};
use super::*;
use crate::telemetry::PeerLive;

fn offline_peer() -> PeerLive {
    PeerLive {
        online: false,
        path: None,
        rtt_ms: None,
        last_seen: Instant::now(),
    }
}

#[test]
fn build_peers_exit_from_announcements() {
    let my_id = pair(1);
    let b = pair(2);
    let c = pair(3);
    let d = pair(4);
    let e = pair(5);

    let mut inputs = base(my_id);
    inputs.exit_on = true;
    inputs.members = [b, c, d].into_iter().collect();
    // E is not in members (offline) but is known to telemetry — otherwise build_peers will not see its row.
    inputs.telemetry.peers.insert(e, offline_peer());
    inputs.exits = [(b, true), (c, false), (e, true)].into_iter().collect();

    let peers = build_peers(&inputs);
    let exit_of = |id: PeerId| {
        peers
            .iter()
            .find(|p| p.id == output::short_peer_id(&id))
            .unwrap_or_else(|| panic!("no row for {id}"))
            .exit
    };

    assert_eq!(exit_of(my_id), Some(true), "own row — Some(exit_on)");
    assert_eq!(exit_of(b), Some(true), "online B with exits[B]=true");
    assert_eq!(exit_of(c), Some(false), "online C with exits[C]=false");
    assert_eq!(exit_of(d), None, "online D without a record in exits");
    assert_eq!(exit_of(e), None, "offline E, even with true in exits");
}

// The `exit.clients` field: the registry returns
// a raw unordered list, while the intersection with network members, clearing when the exit is off,
// sorting and deduplication are done by `build`.
mod exit_clients {
    use super::*;

    #[test]
    fn clients_only_members() {
        // given: the exit registry named a member and a stranger, only the member is in the network members
        let member = id(2);
        let stranger = id(3);
        let opts = Opts {
            exit_on: true,
            members: vec![member],
            exit_clients: vec![member, stranger],
        };

        // when: the snapshot is built
        let clients = snapshot(opts).exit.clients;

        // then: `exit.clients` has only the network member
        assert_eq!(
            clients,
            vec![member.to_string()],
            "the stranger id is filtered out"
        );
    }

    #[test]
    fn clients_empty_when_exit_off() {
        // given: the exit is off, but the registry still remembers a client from the EXIT_CLIENT_LINGER tail
        let member = id(2);
        let opts = Opts {
            exit_on: false,
            members: vec![member],
            exit_clients: vec![member],
        };

        // when/then: the snapshot has no clients
        let clients = snapshot(opts).exit.clients;

        assert!(
            clients.is_empty(),
            "with the exit off there are no clients: {clients:?}"
        );
    }

    #[test]
    fn clients_sorted_and_unique() {
        // given: three network members in descending string order and a repeat of one of them
        let mut members = vec![id(2), id(3), id(4)];
        members.sort_by_key(|m| std::cmp::Reverse(m.to_string()));
        let mut raw = members.clone();
        raw.push(members[1]);
        let opts = Opts {
            exit_on: true,
            members: members.clone(),
            exit_clients: raw,
        };

        // when: the snapshot is built
        let clients = snapshot(opts).exit.clients;

        // then: ascending by string and without repeats
        let mut want: Vec<String> = members.iter().map(|m| m.to_string()).collect();
        want.sort();
        assert_eq!(clients, want);
    }

    #[test]
    fn clients_serialized_under_exit() {
        // given: a snapshot with one client and a snapshot without clients
        let member = id(2);
        let with_client = snapshot(Opts {
            exit_on: true,
            members: vec![member],
            exit_clients: vec![member],
        });
        let without = snapshot(Opts {
            exit_on: true,
            members: vec![member],
            exit_clients: vec![],
        });

        // when: the snapshots are turned into JSON
        let json_with = serde_json::to_value(&with_client).unwrap();
        let json_without = serde_json::to_value(&without).unwrap();

        // then: `exit.clients` is an array of `id_full` strings, and when empty it is `[]`, not absent
        assert_eq!(
            json_with["exit"]["clients"],
            serde_json::json!([member.to_string()])
        );
        assert_eq!(
            json_without["exit"]["clients"],
            serde_json::json!([]),
            "an empty list is `[]`, the key must be present"
        );
    }

    struct Opts {
        exit_on: bool,
        members: Vec<PeerId>,
        exit_clients: Vec<PeerId>,
    }

    /// Returns the snapshot of node 1 under the given preconditions.
    fn snapshot(opts: Opts) -> NodeSnapshot {
        let my_id = pair(1);
        let mut inputs = base(my_id);
        inputs.exit_on = opts.exit_on;
        inputs.members = opts.members.into_iter().collect();
        inputs.exit_clients = opts.exit_clients;
        build(inputs).snap
    }

    fn id(seed: u8) -> PeerId {
        pair(seed)
    }
}

mod exit_rates {
    use super::*;
    use crate::exit::PeerRate;

    fn peer_rows(rates: &[(PeerId, PeerRate)]) -> Vec<crate::web::PeerInfo> {
        let mut inputs = base(pair(1));
        inputs.members = [pair(2)].into_iter().collect();
        inputs.exit_rates = rates.iter().copied().collect();
        build_peers(&inputs)
    }

    fn row(peers: &[crate::web::PeerInfo], id: PeerId) -> &crate::web::PeerInfo {
        let short = output::short_peer_id(&id);
        peers.iter().find(|p| p.id == short).expect("the row exists")
    }

    #[test]
    fn peer_row_carries_exit_rates() {
        let rate = PeerRate { through_peer_bps: 7, through_us_bps: 9 };

        let peers = peer_rows(&[(pair(2), rate)]);

        let x = row(&peers, pair(2));
        assert_eq!((x.through_peer_bps, x.through_us_bps), (7, 9));
    }

    #[test]
    fn peer_without_rates_shows_zero() {
        let peers = peer_rows(&[]);

        let x = row(&peers, pair(2));
        assert_eq!((x.through_peer_bps, x.through_us_bps), (0, 0));
    }

    #[test]
    fn self_row_is_always_zero() {
        let rate = PeerRate { through_peer_bps: 7, through_us_bps: 9 };

        let peers = peer_rows(&[(pair(1), rate)]);

        let me = row(&peers, pair(1));
        assert_eq!((me.through_peer_bps, me.through_us_bps), (0, 0));
    }
}
