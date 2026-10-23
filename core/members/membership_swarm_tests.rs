//! Tests of `run_swarm_events`: counting peers by topic events, subscriber lag and the end of
//! the event stream. No network — a `broadcast` channel and the task itself; waits are up to `STEP`, no blind pauses.

use super::run_swarm_events;
use crate::transport::net::contract::{pid, STEP};
use crate::transport::net::SwarmEvent;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// Polling period of the counter in waits.
const POLL: Duration = Duration::from_millis(5);

mod neighbor_counter {
    use super::*;

    mod up_and_down {
        use super::*;

        #[tokio::test]
        async fn counts_neighbors_up_and_down() {
            // given: the task is started, no peers
            let swarm = setup(Opts::default());
            // when: node a came up, node b came up, node a went down
            swarm.when_events([up(1), up(2), down(1)]);
            // then: one peer remains
            assert_eq!(
                swarm.then_final_neighbors().await,
                1,
                "after Up a, Up b, Down a one peer must remain"
            );
        }
    }

    mod repeated_up {
        use super::*;

        #[tokio::test]
        async fn repeated_up_counts_once() {
            // given: the task is started, no peers
            let swarm = setup(Opts::default());
            // when: Up of the same node came twice
            swarm.when_events([up(1), up(1)]);
            // then: one peer — counted by the set of nodes, not by the number of events
            assert_eq!(
                swarm.then_final_neighbors().await,
                1,
                "a repeated Up of the same node must not increase the count"
            );
        }
    }
}

mod lagged_receiver {
    use super::*;

    #[tokio::test]
    async fn lagged_receiver_keeps_running() {
        // given: a channel of capacity 1, three events in it before the task starts — the first read sees the lag
        let swarm = setup(Opts {
            capacity: 1,
            queued: vec![up(1), up(2), up(3)],
        });
        // when: the task survived the lag and reached the surviving event (count at least 1)
        let before = swarm.when_neighbors_at_least(1).await;
        // then: the task is alive and keeps reading — a new peer is added to the count
        swarm.then_still_running();
        swarm.when_events([up(4)]);
        assert_eq!(
            swarm.then_final_neighbors().await,
            before + 1,
            "after the lag the task must handle the next Up"
        );
    }
}

mod end_of_stream {
    use super::*;

    mod closed_event {
        use super::*;

        #[tokio::test]
        async fn closed_event_stops_loop() {
            // given: the task is started, the channel sender is alive
            let mut swarm = setup(Opts::default());
            // when: a Closed event came
            swarm.when_events([SwarmEvent::Closed("x".to_string())]);
            // then: the task ended within STEP though the channel is not closed
            swarm.then_task_ends().await;
        }
    }

    mod sender_dropped {
        use super::*;

        #[tokio::test]
        async fn sender_drop_stops_loop() {
            // given: the task is started
            let mut swarm = setup(Opts::default());
            // when: the channel sender was dropped
            swarm.when_sender_dropped();
            // then: the task ended within STEP
            swarm.then_task_ends().await;
        }
    }
}

/// Test preconditions: the channel capacity and the events queued in it before the task starts.
struct Opts {
    capacity: usize,
    queued: Vec<SwarmEvent>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            capacity: 16,
            queued: Vec::new(),
        }
    }
}

/// The `run_swarm_events` task on an event channel and its peer counter.
struct Swarm {
    tx: Option<broadcast::Sender<SwarmEvent>>,
    neighbors: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

/// A channel of the needed capacity, events from `opts.queued` are sent before the task starts, then the task is started.
fn setup(opts: Opts) -> Swarm {
    let (tx, rx) = broadcast::channel::<SwarmEvent>(opts.capacity);
    for event in opts.queued {
        tx.send(event)
            .expect("receiver is alive until handed to the task");
    }
    let neighbors = Arc::new(AtomicUsize::new(0));
    let task = crate::node::output::spawn(run_swarm_events(rx, neighbors.clone()));
    Swarm {
        tx: Some(tx),
        neighbors,
        task,
    }
}

impl Swarm {
    fn when_events(&self, events: impl IntoIterator<Item = SwarmEvent>) {
        let tx = self.tx.as_ref().expect("sender already dropped");
        for event in events {
            tx.send(event).expect("the task still holds the receiver");
        }
    }

    fn when_sender_dropped(&mut self) {
        self.tx = None;
    }

    /// Waits up to `STEP` until the counter is at least `n`; returns its value.
    async fn when_neighbors_at_least(&self, n: usize) -> usize {
        let polling = async {
            loop {
                let now = self.neighbors.load(Ordering::Relaxed);
                if now >= n {
                    return now;
                }
                tokio::time::sleep(POLL).await;
            }
        };
        match tokio::time::timeout(STEP, polling).await {
            Ok(now) => now,
            Err(_) => panic!("peer counter did not reach {n} within {STEP:?}"),
        }
    }

    fn then_still_running(&self) {
        assert!(!self.task.is_finished(), "the task ended too early");
    }

    /// The task ends by itself within `STEP` (a task panic is also a failure).
    async fn then_task_ends(&mut self) {
        match tokio::time::timeout(STEP, &mut self.task).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => panic!("the task panicked: {e}"),
            Err(_) => panic!("the task did not end within {STEP:?}"),
        }
    }

    /// Drops the sender and waits for the task to end: by then all sent events are parsed.
    async fn then_final_neighbors(mut self) -> usize {
        self.when_sender_dropped();
        self.then_task_ends().await;
        self.neighbors.load(Ordering::Relaxed)
    }
}

fn up(seed: u8) -> SwarmEvent {
    SwarmEvent::NeighborUp(pid(seed))
}

fn down(seed: u8) -> SwarmEvent {
    SwarmEvent::NeighborDown(pid(seed))
}
