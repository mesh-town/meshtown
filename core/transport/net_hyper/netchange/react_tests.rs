//! Reaction to a network change: events from a channel, calls are recorded by `Recorder`; time is paused.

use super::*;
use crate::transport::net::transport::{NET_CHANGED, NET_LOST};
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Close(u32, String),
    Suspend,
    Resume,
}

struct ChanEvents(mpsc::UnboundedReceiver<NetEvent>);

impl NetEvents for ChanEvents {
    fn next(&mut self) -> BoxFuture<'_, Option<NetEvent>> {
        Box::pin(async { self.0.recv().await })
    }
}

#[derive(Clone, Default)]
struct Recorder {
    calls: Arc<Mutex<Vec<Call>>>,
    resume_at: Arc<Mutex<Option<Instant>>>,
    suspend_hold: Duration,
    resume_fails: bool,
}

impl Recorder {
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    fn push(&self, c: Call) {
        self.calls.lock().unwrap().push(c);
    }
}

impl NetReact for Recorder {
    fn close_sessions(&self, code: u32, reason: &[u8]) -> usize {
        self.push(Call::Close(
            code,
            String::from_utf8_lossy(reason).into_owned(),
        ));
        1
    }
    fn suspend(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async {
            self.push(Call::Suspend);
            sleep(self.suspend_hold).await;
            Ok(())
        })
    }
    fn resume(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async {
            self.push(Call::Resume);
            *self.resume_at.lock().unwrap() = Some(Instant::now());
            if self.resume_fails {
                return Err("resume failed".to_string());
            }
            Ok(())
        })
    }
}

struct Rig {
    tx: mpsc::UnboundedSender<NetEvent>,
    stop: watch::Sender<bool>,
    rec: Recorder,
    task: JoinHandle<()>,
}

fn rig(rec: Recorder) -> Rig {
    let (tx, rx) = mpsc::unbounded_channel();
    let (stop, stop_rx) = watch::channel(false);
    let task = crate::node::output::spawn(react_loop(
        Box::new(ChanEvents(rx)),
        Arc::new(rec.clone()),
        stop_rx,
    ));
    Rig {
        tx,
        stop,
        rec,
        task,
    }
}

fn closed(code: u32, reason: &str) -> Call {
    Call::Close(code, reason.to_string())
}

fn triple() -> Vec<Call> {
    vec![
        closed(CODE_NET_CHANGED, NET_CHANGED),
        Call::Suspend,
        Call::Resume,
    ]
}

#[tokio::test(start_paused = true)]
async fn changed_closes_then_suspends_then_resumes() {
    let r = rig(Recorder::default());
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(Duration::from_secs(5)).await;
    assert_eq!(r.rec.calls(), triple());
}

#[tokio::test(start_paused = true)]
async fn burst_inside_settle_reacts_once() {
    let r = rig(Recorder::default());
    for _ in 0..3 {
        r.tx.send(NetEvent::Changed).unwrap();
        sleep(Duration::from_millis(100)).await;
    }
    sleep(SETTLE_WINDOW + Duration::from_secs(2)).await;
    assert_eq!(r.rec.calls(), triple());
}

#[tokio::test(start_paused = true)]
async fn change_during_reaction_reacts_once_more() {
    let rec = Recorder {
        suspend_hold: Duration::from_secs(1),
        ..Default::default()
    };
    let r = rig(rec);
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(SETTLE_WINDOW + Duration::from_millis(300)).await;
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(Duration::from_secs(10)).await;
    let mut want = triple();
    want.extend(triple());
    assert_eq!(r.rec.calls(), want);
}

#[tokio::test(start_paused = true)]
async fn reaction_ends_within_settle_plus_margin() {
    let r = rig(Recorder::default());
    let t0 = Instant::now();
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(Duration::from_secs(3)).await;
    let at = r.rec.resume_at.lock().unwrap().expect("resume happened");
    assert!(
        at <= t0 + SETTLE_WINDOW + Duration::from_millis(50),
        "resume too late: {:?}",
        at - t0
    );
}

#[tokio::test(start_paused = true)]
async fn lost_closes_and_suspends_without_resume() {
    let r = rig(Recorder::default());
    r.tx.send(NetEvent::Lost).unwrap();
    sleep(Duration::from_secs(60)).await;
    assert_eq!(
        r.rec.calls(),
        vec![closed(CODE_NET_CHANGED, NET_LOST), Call::Suspend]
    );
}

#[tokio::test(start_paused = true)]
async fn lost_twice_suspends_once() {
    let r = rig(Recorder::default());
    r.tx.send(NetEvent::Lost).unwrap();
    sleep(Duration::from_secs(2)).await;
    r.tx.send(NetEvent::Lost).unwrap();
    sleep(Duration::from_secs(2)).await;
    let suspends = r
        .rec
        .calls()
        .iter()
        .filter(|c| **c == Call::Suspend)
        .count();
    assert_eq!(suspends, 1);
}

#[tokio::test(start_paused = true)]
async fn changed_after_lost_resumes_without_second_suspend() {
    let r = rig(Recorder::default());
    r.tx.send(NetEvent::Lost).unwrap();
    sleep(Duration::from_secs(2)).await;
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(Duration::from_secs(2)).await;
    let want = vec![
        closed(CODE_NET_CHANGED, NET_LOST),
        Call::Suspend,
        closed(CODE_NET_CHANGED, NET_CHANGED),
        Call::Resume,
    ];
    assert_eq!(r.rec.calls(), want);
}

#[tokio::test(start_paused = true)]
async fn lost_inside_settle_after_changed_wins() {
    let r = rig(Recorder::default());
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(Duration::from_millis(100)).await;
    r.tx.send(NetEvent::Lost).unwrap();
    sleep(Duration::from_secs(5)).await;
    assert_eq!(
        r.rec.calls(),
        vec![closed(CODE_NET_CHANGED, NET_LOST), Call::Suspend]
    );
}

#[tokio::test(start_paused = true)]
async fn resume_error_keeps_loop_alive() {
    let r = rig(Recorder {
        resume_fails: true,
        ..Default::default()
    });
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(Duration::from_secs(2)).await;
    r.tx.send(NetEvent::Changed).unwrap();
    sleep(Duration::from_secs(2)).await;
    let mut want = triple();
    want.extend(triple());
    assert_eq!(r.rec.calls(), want);
    assert!(!r.task.is_finished());
}

#[tokio::test(start_paused = true)]
async fn stop_ends_loop_without_calls() {
    let r = rig(Recorder::default());
    r.stop.send(true).unwrap();
    sleep(Duration::from_secs(1)).await;
    assert!(r.task.is_finished());
    assert!(r.rec.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn source_end_ends_loop() {
    let r = rig(Recorder::default());
    drop(r.tx);
    sleep(Duration::from_secs(1)).await;
    assert!(r.task.is_finished());
}
