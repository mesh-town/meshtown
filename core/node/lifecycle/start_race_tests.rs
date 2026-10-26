//! The start race: a start that a stop interrupts, and what becomes of the abandoned start.
//!
//! The node type is `u32`; a "hanging" start is a future that never ends (or ends behind a gate).
//! Time is paused, so the whole suite runs without real waiting; every wait on the race sits under
//! an outer `timeout`, so a wrong implementation fails instead of hanging.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout, Instant};

use super::{start_unless_stopped, ABANDONED_START_LIMIT, STOP_POLL};
use crate::config::config_file::Settings;
use crate::node::node_ctl::{Cmd, NodeCtl};
use crate::web_gui::NodeControl;

const OUTER_LIMIT: Duration = Duration::from_secs(5);
const STOP_AFTER: Duration = Duration::from_millis(10);

mod without_a_stop {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn start_without_stop_returns_the_node() {
        let mut fx = setup();

        let res = fx.race(async { Ok(7) }).await;

        assert_eq!(res.unwrap(), Some(7));
        fx.then_nothing_is_discarded();
    }

    #[tokio::test(start_paused = true)]
    async fn start_error_without_stop_is_returned() {
        let fx = setup();

        let res = fx.race(async { Err(anyhow::anyhow!("boom")) }).await;

        let err = res.expect_err("the start error must come out");
        assert!(format!("{err:#}").contains("boom"), "error: {err:#}");
    }

    #[tokio::test(start_paused = true)]
    #[should_panic(expected = "boom")]
    async fn start_panic_is_propagated() {
        let fx = setup();

        let _ = fx.race(async { panic!("boom") }).await;
    }

    #[tokio::test(start_paused = true)]
    async fn dropped_race_aborts_the_start() {
        // given: a hanging start that raises a flag when its future is destroyed
        let fx = setup();
        let (flag, start) = hanging_with_guard();

        // when: the race is destroyed from outside (no stop was ever requested)
        let res = timeout(Duration::from_millis(10), fx.race(start)).await;
        settle().await;

        // then: the start is aborted, not left running unattended
        assert!(res.is_err(), "the race must still have been waiting");
        assert!(
            flag.load(Ordering::SeqCst),
            "the start future must be dropped"
        );
    }
}

mod stop_during_a_hanging_start {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn signal_stop_during_a_hanging_start_returns_none() {
        let fx = setup();

        let (res, took) = fx
            .when_stopped_after_10ms(|ctl| ctl.stop_on_os_signal())
            .await;

        then_interrupted_quickly(res, took);
    }

    #[tokio::test(start_paused = true)]
    async fn page_stop_during_a_hanging_start_returns_none() {
        let fx = setup();

        let (res, took) = fx.when_stopped_after_10ms(|ctl| ctl.stop()).await;

        then_interrupted_quickly(res, took);
    }

    #[tokio::test(start_paused = true)]
    async fn leave_stop_during_a_hanging_start_returns_none() {
        let fx = setup();

        let (res, took) = fx.when_stopped_after_10ms(|ctl| ctl.stop_for_leave()).await;

        then_interrupted_quickly(res, took);
    }

    #[tokio::test(start_paused = true)]
    async fn stop_after_a_queued_switch_still_interrupts() {
        // given: a hanging start and a network switch already waiting in the queue
        let fx = setup();
        fx.ctl
            .switch_network(&crate::node::host::test_keys::managed_text(1));

        // when: a stop comes after it
        let (res, _) = fx
            .when_stopped_after_10ms(|ctl| ctl.stop_on_os_signal())
            .await;

        // then: the start is interrupted, and the queue folds to the stop
        assert!(res.unwrap().is_none(), "the stop must interrupt the start");
        assert_eq!(fx.next_command().await, Cmd::Stop);
    }

    #[tokio::test(start_paused = true)]
    async fn interrupted_start_leaves_the_stop_in_the_queue() {
        let fx = setup();

        let (res, _) = fx
            .when_stopped_after_10ms(|ctl| ctl.stop_on_os_signal())
            .await;

        assert!(res.unwrap().is_none());
        assert_eq!(fx.next_command().await, Cmd::Stop);
    }
}

mod start_and_stop_at_once {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn start_done_before_the_first_check_wins_over_stop() {
        // given: a stop is already requested and the start is ready at once
        let mut fx = setup();
        fx.ctl.stop_on_os_signal();

        // when
        let res = fx.race(async { Ok(5) }).await;

        // then: the node comes out (the stop stays in the queue for the serving loop)
        assert_eq!(res.unwrap(), Some(5));
        fx.then_nothing_is_discarded();
    }
}

mod abandoned_start {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn late_start_after_stop_is_discarded() {
        // given: a start behind a gate, a stop, and the race that already returned `None`
        let mut fx = setup();
        let (open, start) = gated_start(Ok(9));
        fx.ctl.stop_on_os_signal();
        assert!(fx.race(start).await.unwrap().is_none());

        // when: the start ends
        open.send(()).unwrap();

        // then: the node it produced is handed to `discard`
        let got = timeout(OUTER_LIMIT, fx.discarded.recv()).await;
        assert_eq!(got.expect("discard must be called"), Some(9));
    }

    #[tokio::test(start_paused = true)]
    async fn late_failure_after_stop_discards_nothing() {
        // given: the same, but the start will fail
        let mut fx = setup();
        let (open, start) = gated_start(Err(anyhow::anyhow!("late")));
        fx.ctl.stop_on_os_signal();
        assert!(fx.race(start).await.unwrap().is_none());

        // when: the start ends with an error and some time passes
        open.send(()).unwrap();
        sleep(STOP_POLL * 10).await;

        // then: there is nothing to discard
        fx.then_nothing_is_discarded();
    }

    #[tokio::test(start_paused = true)]
    async fn abandoned_start_is_dropped_after_the_limit() {
        // given: a hanging start that raises a flag when destroyed, a stop, and the race returned `None`
        let fx = setup();
        let (flag, start) = hanging_with_guard();
        fx.ctl.stop_on_os_signal();
        assert!(fx.race(start).await.unwrap().is_none());
        sleep(ABANDONED_START_LIMIT - Duration::from_secs(1)).await;
        assert!(
            !flag.load(Ordering::SeqCst),
            "the start must get its whole limit"
        );

        // when: the limit passes
        sleep(Duration::from_secs(2)).await;
        settle().await;

        // then: the start future is destroyed
        assert!(flag.load(Ordering::SeqCst), "the start must be aborted");
    }
}

fn then_interrupted_quickly(res: anyhow::Result<Option<u32>>, took: Duration) {
    assert!(res.unwrap().is_none(), "the stop must interrupt the start");
    assert!(
        took <= STOP_AFTER + STOP_POLL * 2,
        "the stop was noticed after {took:?}"
    );
}

struct Fixture {
    ctl: Arc<NodeCtl>,
    discard_tx: mpsc::UnboundedSender<u32>,
    discarded: mpsc::UnboundedReceiver<u32>,
}

fn setup() -> Fixture {
    let (discard_tx, discarded) = mpsc::unbounded_channel();
    Fixture {
        ctl: NodeCtl::new(&Settings::default()),
        discard_tx,
        discarded,
    }
}

impl Fixture {
    /// Runs the race with a `discard` that reports into the fixture; gives up after `OUTER_LIMIT`.
    async fn race(
        &self,
        start: impl Future<Output = anyhow::Result<u32>> + Send + 'static,
    ) -> anyhow::Result<Option<u32>> {
        let tx = self.discard_tx.clone();
        let race = start_unless_stopped(&self.ctl, start, move |node| {
            Box::pin(async move {
                let _ = tx.send(node);
            })
        });
        timeout(OUTER_LIMIT, race).await.expect("the race must end")
    }

    /// A hanging start; `stop` fires on the control object after 10 ms. Returns the result and the elapsed time.
    async fn when_stopped_after_10ms(
        &self,
        stop: impl FnOnce(&NodeCtl) + Send + 'static,
    ) -> (anyhow::Result<Option<u32>>, Duration) {
        let ctl = self.ctl.clone();
        crate::node::output::spawn(async move {
            sleep(STOP_AFTER).await;
            stop(&ctl);
        });
        let began = Instant::now();
        let res = self.race(std::future::pending()).await;
        (res, began.elapsed())
    }

    async fn next_command(&self) -> Cmd {
        timeout(OUTER_LIMIT, self.ctl.next_command())
            .await
            .expect("a command must be waiting in the queue")
    }

    fn then_nothing_is_discarded(&mut self) {
        let got = self.discarded.try_recv();
        assert!(got.is_err(), "discard must not be called, got {got:?}");
    }
}

/// A start that ends with `outcome` once the returned sender fires.
fn gated_start(
    outcome: anyhow::Result<u32>,
) -> (
    oneshot::Sender<()>,
    impl Future<Output = anyhow::Result<u32>> + Send + 'static,
) {
    let (open, gate) = oneshot::channel::<()>();
    let start = async move {
        gate.await.expect("the gate is opened");
        outcome
    };
    (open, start)
}

/// Raised when the guard is dropped.
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// A start that never ends and owns a guard: the flag rises when the future is destroyed.
fn hanging_with_guard() -> (
    Arc<AtomicBool>,
    impl Future<Output = anyhow::Result<u32>> + Send + 'static,
) {
    let flag = Arc::new(AtomicBool::new(false));
    let guard = DropFlag(flag.clone());
    let start = async move {
        let _guard = guard;
        std::future::pending::<anyhow::Result<u32>>().await
    };
    (flag, start)
}

/// Lets the spawned tasks run (an abort takes effect when the task is next scheduled).
async fn settle() {
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
}
