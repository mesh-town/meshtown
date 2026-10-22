//! Tests for the gossip-topic re-invite (`node/rejoin.rs`): the pause schedule (`Rejoin`) and
//! the `rejoin_if_alone` decision on a `Membership` double. Time comes in as an argument from
//! `t0 = Instant::now()` plus manual offsets, with no sleeping — the whole file takes < 1 s.

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::broadcast;
use tokio::time::Instant;

use crate::membership::Members;
use crate::net::{BoxFuture, Membership, NetError, NetResult, NodeSecret, PeerId, SwarmEvent};

use super::{rejoin_if_alone, Rejoin};

mod fresh_schedule {
    use super::*;

    #[test]
    fn not_due_before_first_delay() {
        // given: a schedule created at t0
        let t0 = Instant::now();
        let rejoin = Rejoin::new(t0);

        // when: checking one second before the deadline and exactly at the deadline (0 peers, 2 members)
        let early = rejoin.is_due(at(t0, 29), 0, 2);
        let on_time = rejoin.is_due(at(t0, 30), 0, 2);

        // then: no retry before 30 s, due at 30 s
        assert!(!early, "retry must not be due at second 29");
        assert!(on_time, "retry must be due at second 30");
    }

    #[test]
    fn not_due_with_neighbors_or_without_members() {
        // given: a schedule whose deadline has come (t0 + 30 s)
        let t0 = Instant::now();
        let rejoin = Rejoin::new(t0);

        // when: there is a topic peer / there are no members, nobody to call
        let with_neighbor = rejoin.is_due(at(t0, 30), 1, 2);
        let without_members = rejoin.is_due(at(t0, 30), 0, 0);

        // then: no retry in either case
        assert!(!with_neighbor, "no retry needed when a topic peer exists");
        assert!(!without_members, "nobody to call without members");
    }
}

mod alone_for_twenty_minutes {
    use super::*;

    #[test]
    fn backoff_doubles_to_five_minutes() {
        // given: a schedule at t0, 0 peers, 2 members
        let t0 = Instant::now();
        let mut rejoin = Rejoin::new(t0);
        let mut moments = Vec::new();

        // when: a tick every 30 s up to 1100 s; on each `is_due` — `mark_rejoined`
        for sec in (30..=1100).step_by(30) {
            if rejoin.is_due(at(t0, sec), 0, 2) {
                rejoin.mark_rejoined(at(t0, sec));
                moments.push(sec);
            }
        }

        // then: the pause is 30, 60, 120, 240, then the 300 s ceiling
        assert_eq!(
            moments,
            vec![30, 90, 210, 450, 750, 1050],
            "retry moments since start, s"
        );
    }
}

mod after_two_repeats {
    use super::*;

    #[test]
    fn reset_restores_first_delay() {
        // given: two retries (at 30 and 90 s), the pause grew to 120 s, the next deadline is 210 s
        let t0 = Instant::now();
        let mut rejoin = Rejoin::new(t0);
        rejoin.mark_rejoined(at(t0, 30));
        rejoin.mark_rejoined(at(t0, 90));

        // when: peers appeared, `reset_pause` at t0 + 100 s
        let reset_at = at(t0, 100);
        rejoin.reset_pause(reset_at);

        // then: the deadline is 30 s after the reset (130 s), not 210 s
        assert!(
            !rejoin.is_due(reset_at + Duration::from_secs(29), 0, 2),
            "retry must not be due until 30 s after the reset"
        );
        assert!(
            rejoin.is_due(reset_at + Duration::from_secs(30), 0, 2),
            "retry must be due 30 s after the reset"
        );
    }
}

mod tick_alone_with_members {
    use super::*;

    #[tokio::test]
    async fn joins_all_members_when_alone() {
        // given: members a and b, 0 topic peers
        let mut f = setup(Opts::default());

        // when: a tick at t0 + 30 s, then another at t0 + 31 s
        f.when_tick(30, 0).await;
        f.when_tick(31, 0).await;

        // then: exactly one invite, and it holds both members without duplicates
        let joins = f.then_joins();
        assert_eq!(joins.len(), 1, "invites: {joins:?}");
        assert_eq!(
            joins[0].len(),
            2,
            "the list must hold exactly two members"
        );
        let invited: HashSet<PeerId> = joins[0].iter().copied().collect();
        assert_eq!(
            invited,
            [f.a, f.b].into_iter().collect(),
            "wrong members invited"
        );
    }
}

mod tick_with_neighbors {
    use super::*;

    #[tokio::test]
    async fn skips_when_neighbors() {
        // given: members a and b, one topic peer
        let mut f = setup(Opts::default());

        // when: a tick at t0 + 30 s
        f.when_tick(30, 1).await;

        // then: no invites
        let joins = f.then_joins();
        assert!(joins.is_empty(), "no need to call with a peer present: {joins:?}");
    }
}

mod tick_when_join_fails {
    use super::*;

    #[tokio::test]
    async fn join_error_is_not_fatal() {
        // given: members a and b, 0 peers, `join` answers with an error
        let mut f = setup(Opts { join_fails: true });

        // when: a tick at t0 + 30 s (returns without panic), then at 89 and 90 s
        f.when_tick(30, 0).await;
        let after_error = f.then_joins().len();
        f.when_tick(89, 0).await;
        let at_89 = f.then_joins().len();
        f.when_tick(90, 0).await;
        let at_90 = f.then_joins().len();

        // then: there was one attempt, the next retry is no earlier than t0 + 90 s, and it is there at 90 s
        assert_eq!(after_error, 1, "the invite at 30 s must be sent");
        assert_eq!(
            at_89, 1,
            "after an error the deadline shifts: no retry until 90 s"
        );
        assert_eq!(at_90, 2, "the retry must happen at 90 s");
    }
}

// ---- helpers ----

#[derive(Default)]
struct Opts {
    /// The double's `join` answers with `Err`.
    join_fails: bool,
}

struct Fixture {
    t0: Instant,
    swarm: RecordingSwarm,
    rejoin: Rejoin,
    members: Members,
    a: PeerId,
    b: PeerId,
}

/// A schedule at `t0`, members a and b, a topic double per `opts`.
fn setup(opts: Opts) -> Fixture {
    let t0 = Instant::now();
    let (a, b) = (peer(1), peer(2));
    let members = Members::default();
    members.add_verified_conn(a);
    members.add_verified_conn(b);
    Fixture {
        t0,
        swarm: RecordingSwarm::new(opts.join_fails),
        rejoin: Rejoin::new(t0),
        members,
        a,
        b,
    }
}

impl Fixture {
    /// A node tick `secs` seconds after `t0` with the given number of topic peers.
    async fn when_tick(&mut self, secs: u64, neighbors: usize) {
        let now = at(self.t0, secs);
        rejoin_if_alone(&self.swarm, &mut self.rejoin, neighbors, &self.members, now).await;
    }

    /// The argument lists of all executed `join` calls, in call order.
    fn then_joins(&self) -> Vec<Vec<PeerId>> {
        self.swarm.joins.lock().unwrap().clone()
    }
}

/// A topic double: collects the `join` arguments, answers with an error on a flag.
struct RecordingSwarm {
    joins: Mutex<Vec<Vec<PeerId>>>,
    join_fails: bool,
}

impl RecordingSwarm {
    fn new(join_fails: bool) -> Self {
        Self {
            joins: Mutex::new(Vec::new()),
            join_fails,
        }
    }
}

impl Membership for RecordingSwarm {
    fn join(&self, peers: &[PeerId]) -> BoxFuture<'_, NetResult<()>> {
        let peers = peers.to_vec();
        Box::pin(async move {
            self.joins.lock().unwrap().push(peers);
            if self.join_fails {
                return Err(NetError::Closed);
            }
            Ok(())
        })
    }

    fn broadcast(&self, _msg: Bytes) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn subscribe(&self) -> broadcast::Receiver<SwarmEvent> {
        broadcast::channel(1).1
    }
}

/// The moment `secs` seconds after `t0`.
fn at(t0: Instant, secs: u64) -> Instant {
    t0 + Duration::from_secs(secs)
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}
