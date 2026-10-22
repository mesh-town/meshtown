//! Tests of the tray spinner state (`spin.rs`): the remembered ring size, the life of the ticker, which
//! ticks the core accepts and the ticker's send-or-skip step. Nothing here sleeps: the ticker thread is never
//! spawned, `ticker_step` is called by hand.

use super::*;
use crate::app::view::{NodeState, View};
use crate::app::view_ring::{Ring, RingRole};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[test]
fn tick_period_is_100_ms() {
    assert_eq!(SPIN_TICK, Duration::from_millis(100));
}

/// The sector count of the last running ring, kept for the grey ring of a stopped or starting node.
mod memory {
    use super::*;

    #[test]
    fn default_is_unknown() {
        let idle = Spin::new().idle();
        assert_eq!(idle.remembered, 0, "0 means the count is not known");
        assert_eq!(idle.frame, 0);
    }

    #[test]
    fn running_ring_is_remembered() {
        let mut spin = Spin::new();
        spin.follow(running_with(5));
        assert_eq!(spin.idle().remembered, 5);
    }

    #[test]
    fn counts_all_sectors_not_one_role() {
        let mut spin = Spin::new();
        let mixed = Ring::EMPTY
            .add(RingRole::Online)
            .add(RingRole::Online)
            .add(RingRole::ActiveExit)
            .add(RingRole::ExitClient)
            .add(RingRole::Offline);
        spin.follow(View { ring: mixed, ..view_in(NodeState::Running) });
        assert_eq!(spin.idle().remembered, 5);
    }

    #[test]
    fn empty_ring_keeps_memory() {
        let mut spin = Spin::new();
        spin.follow(running_with(5));
        spin.follow(view_in(NodeState::Running));
        assert_eq!(spin.idle().remembered, 5, "a running view without members does not erase the memory");
    }

    #[test]
    fn latest_running_ring_replaces_memory() {
        let mut spin = Spin::new();
        spin.follow(running_with(5));
        spin.follow(running_with(3));
        assert_eq!(spin.idle().remembered, 3);
    }

    #[test]
    fn survives_stop_and_start() {
        let mut spin = Spin::new();
        spin.follow(running_with(5));
        spin.follow(view_in(NodeState::Absent));
        spin.follow(view_in(NodeState::Starting));
        assert_eq!(spin.idle().remembered, 5);
    }

    #[test]
    fn only_running_ring_is_remembered() {
        // Only a running view with members sets the memory; a ring in any other state is not remembered.
        let mut spin = Spin::new();
        for node in [NodeState::Unknown, NodeState::Absent, NodeState::Starting, NodeState::Stopping] {
            spin.follow(View { ring: Ring::uniform(RingRole::Online, 7), ..view_in(node) });
            assert_eq!(spin.idle().remembered, 0, "{node:?}");
        }
    }
}

/// The ticker exists only while the node is `Starting`: one per entry, a new generation per re-entry.
mod ticker_life {
    use super::*;

    #[test]
    fn starting_entry_returns_ticker_once() {
        let mut spin = Spin::new();
        assert!(spin.follow(view_in(NodeState::Starting)).is_some(), "the first Starting view");
        assert!(spin.follow(view_in(NodeState::Starting)).is_none(), "a repeated Starting view");
    }

    #[test]
    fn new_ticker_is_alive_and_carries_the_generation() {
        let (spin, ticker) = started();
        assert!(ticker.is_alive());
        assert_eq!(ticker.generation, spin.generation());
    }

    #[test]
    fn leaving_starting_kills_ticker() {
        for next in [NodeState::Unknown, NodeState::Absent, NodeState::Running, NodeState::Stopping] {
            let (mut spin, ticker) = started();
            spin.follow(view_in(next));
            assert!(!ticker.is_alive(), "after Starting -> {next:?}");
        }
    }

    #[test]
    fn reentry_bumps_generation() {
        let (mut spin, first) = started();
        let before = spin.generation();
        spin.follow(view_in(NodeState::Absent));

        let second = spin.follow(view_in(NodeState::Starting)).expect("a new ticker on re-entry");

        assert_eq!(spin.generation(), before + 1);
        assert_eq!(second.generation, before + 1);
        assert!(!first.is_alive(), "the old ticker stays dead");
        assert!(second.is_alive());
    }

    #[test]
    fn repeated_starting_keeps_generation() {
        let (mut spin, _ticker) = started();
        let before = spin.generation();
        spin.follow(view_in(NodeState::Starting));
        assert_eq!(spin.generation(), before);
    }

    #[test]
    fn no_ticker_outside_starting() {
        for node in [NodeState::Unknown, NodeState::Absent, NodeState::Running, NodeState::Stopping] {
            let mut spin = Spin::new();
            assert!(spin.follow(view_in(node)).is_none(), "{node:?}");
        }
    }
}

/// `Spin::tick`: the core accepts a tick only from the live ticker of the current generation.
mod ticks {
    use super::*;

    #[test]
    fn tick_of_current_generation_advances_frame() {
        let (mut spin, _ticker) = started();
        assert!(spin.tick(spin.generation()));
        assert_eq!(spin.idle().frame, 1);
        assert!(spin.tick(spin.generation()));
        assert_eq!(spin.idle().frame, 2);
    }

    #[test]
    fn stale_tick_is_ignored() {
        let (mut spin, old) = started();
        spin.follow(view_in(NodeState::Absent));
        spin.follow(view_in(NodeState::Starting));

        assert!(!spin.tick(old.generation), "a tick of the previous start");
        assert_eq!(spin.idle().frame, 0);
    }

    #[test]
    fn late_tick_is_ignored() {
        let (mut spin, _ticker) = started();
        let generation = spin.generation();
        spin.follow(view_in(NodeState::Running));
        let frame = spin.idle().frame;

        assert!(!spin.tick(generation), "a tick that arrives after Starting is over");
        assert_eq!(spin.idle().frame, frame);
    }

    #[test]
    fn tick_without_any_start_is_ignored() {
        let mut spin = Spin::new();
        assert!(!spin.tick(spin.generation()));
        assert_eq!(spin.idle().frame, 0);
    }

    #[test]
    fn entry_resets_frame() {
        let (mut spin, _ticker) = started();
        for _ in 0..3 {
            spin.tick(spin.generation());
        }
        spin.follow(view_in(NodeState::Absent));

        spin.follow(view_in(NodeState::Starting));

        assert_eq!(spin.idle().frame, 0);
    }

    #[test]
    fn repeated_starting_keeps_frame() {
        let (mut spin, _ticker) = started();
        spin.tick(spin.generation());
        spin.follow(view_in(NodeState::Starting));
        assert_eq!(spin.idle().frame, 1, "a second Starting view does not restart the spinner");
    }

    #[test]
    fn accepted_tick_lets_ticker_send_again() {
        // Reads `Ticker.flags` and `Ticker.generation`: the step is what the ticker thread runs with them.
        let (mut spin, ticker) = started();
        let generation = ticker.generation;
        assert_eq!(one_step(&ticker.flags, generation), (true, vec![generation]));
        assert_eq!(one_step(&ticker.flags, generation), (true, vec![]), "one tick is already queued");

        assert!(spin.tick(generation));

        assert_eq!(one_step(&ticker.flags, generation), (true, vec![generation]), "the core took the tick");
    }

    #[test]
    fn ignored_tick_keeps_pending() {
        let (mut spin, old) = started();
        spin.follow(view_in(NodeState::Absent));
        let current = spin.follow(view_in(NodeState::Starting)).expect("a new ticker");
        one_step(&current.flags, current.generation);

        assert!(!spin.tick(old.generation));

        assert_eq!(
            one_step(&current.flags, current.generation),
            (true, vec![]),
            "a rejected tick must not free the slot of the current ticker"
        );
    }
}

/// `ticker_step`: what the ticker thread does after each sleep.
mod step {
    use super::*;

    #[test]
    fn step_stops_when_not_alive() {
        assert_eq!(one_step(&flags(false, false), 4), (false, vec![]));
    }

    #[test]
    fn step_stops_when_not_alive_even_if_pending() {
        assert_eq!(one_step(&flags(false, true), 4), (false, vec![]));
    }

    #[test]
    fn step_sends_generation() {
        assert_eq!(one_step(&flags(true, false), 42), (true, vec![42]));
    }

    #[test]
    fn step_marks_pending_after_send() {
        let shared = flags(true, false);
        one_step(&shared, 42);
        assert!(shared.pending.load(Ordering::SeqCst));
    }

    #[test]
    fn step_coalesces_while_pending() {
        assert_eq!(one_step(&flags(true, true), 42), (true, vec![]), "go on, but send nothing");
    }

    #[test]
    fn three_steps_without_receive_send_one_tick() {
        let shared = flags(true, false);
        let sent: Vec<u64> = (0..3).flat_map(|_| one_step(&shared, 9).1).collect();
        assert_eq!(sent, vec![9], "three steps, one queued tick");
    }
}

/// A view in the given node state with no ring, as the poller sends before the first snapshot.
fn view_in(node: NodeState) -> View {
    View { node, ..View::initial() }
}

/// A running view whose ring has `sectors` online members.
fn running_with(sectors: u16) -> View {
    View { ring: Ring::uniform(RingRole::Online, sectors), ..view_in(NodeState::Running) }
}

/// A fresh `Spin` that has just seen the first `Starting` view, and the ticker it returned.
fn started() -> (Spin, Ticker) {
    let mut spin = Spin::new();
    let ticker = spin.follow(view_in(NodeState::Starting)).expect("entering Starting gives a ticker");
    (spin, ticker)
}

/// Ticker flags in the given state.
fn flags(alive: bool, pending: bool) -> TickerFlags {
    // A struct literal: `TickerFlags` has no constructor, only the two flags.
    TickerFlags { alive: AtomicBool::new(alive), pending: AtomicBool::new(pending) }
}

/// One `ticker_step`: whether the thread goes on, and the generations it sent to the core.
fn one_step(flags: &TickerFlags, generation: u64) -> (bool, Vec<u64>) {
    let mut sent = Vec::new();
    let go_on = ticker_step(flags, generation, &mut |g| sent.push(g));
    (go_on, sent)
}
