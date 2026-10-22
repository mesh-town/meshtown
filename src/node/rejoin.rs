//! Re-invites members into the gossip topic while the topic has no peers but members exist.
//!
//! What it does: on a node tick, decides whether it is time to call the members into the topic again (`join`); the pause
//! doubles from 30 s up to 5 min, and the appearance of peers resets it to 30 s.
//! What it doesn't do: it owns neither the timer nor the clock — time comes in as an argument; it knows the topic and network
//! only through `Membership`.
//! Links: `event_loop::run_node_loop` calls `rejoin_if_alone` on the `broadcast_tick` tick.

use std::time::Duration;

use tokio::time::Instant;

use crate::membership::Members;
use crate::net::Membership;
use crate::output::say;

/// The pause before the first retry and after peers appear.
pub(super) const REJOIN_FIRST: Duration = Duration::from_secs(30);
/// The ceiling of the pause between retries.
pub(super) const REJOIN_MAX: Duration = Duration::from_secs(300);

/// The retry schedule: the next deadline and the current pause.
pub(super) struct Rejoin {
    next_at: Instant,
    pause: Duration,
}

impl Rejoin {
    /// The first deadline is `REJOIN_FIRST` from `now`.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            next_at: now + REJOIN_FIRST,
            pause: REJOIN_FIRST,
        }
    }

    /// Whether it is time to retry: 0 peers, members exist, the deadline has come.
    pub(super) fn is_due(&self, now: Instant, neighbor_count: usize, member_count: usize) -> bool {
        neighbor_count == 0 && member_count > 0 && now >= self.next_at
    }

    /// A retry was made: the pause doubles (up to the ceiling), the deadline is `now` + the pause.
    pub(super) fn mark_rejoined(&mut self, now: Instant) {
        self.pause = (self.pause * 2).min(REJOIN_MAX);
        self.next_at = now + self.pause;
    }

    /// Peers exist: the pause is `REJOIN_FIRST` again, the deadline is that far from `now`.
    pub(super) fn reset_pause(&mut self, now: Instant) {
        self.pause = REJOIN_FIRST;
        self.next_at = now + REJOIN_FIRST;
    }
}

/// With 0 peers and the deadline reached by the schedule, invites all members into the topic; an error is not fatal.
pub(super) async fn rejoin_if_alone(
    swarm: &dyn Membership,
    rejoin: &mut Rejoin,
    neighbor_count: usize,
    members: &Members,
    now: Instant,
) {
    if neighbor_count > 0 {
        return rejoin.reset_pause(now);
    }
    if !rejoin.is_due(now, neighbor_count, members.count()) {
        return;
    }
    rejoin.mark_rejoined(now);
    let ids: Vec<_> = members.ids().into_iter().collect();
    say(
        "gossip",
        format!(
            "0 peers — re-inviting {} members to the topic",
            ids.len()
        ),
    );
    if let Err(e) = swarm.join(&ids).await {
        say("gossip", format!("couldn't re-invite: {e}"));
    }
}

#[cfg(test)]
#[path = "rejoin_tests.rs"]
mod tests;
