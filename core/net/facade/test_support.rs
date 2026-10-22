//! Helpers for the tests of the facade, the registry and discovery merging: the scripted source `Script`,
//! the spy transport `Spy`, the probe handler `Probe`, the shared order journal `Journal`.

use crate::net::addr::{AddrHint, PeerAddr, RelayRef};
use crate::net::discovery::Discovery;
use crate::net::error::{NetError, NetResult};
use crate::net::facade::ServiceHandler;
use crate::net::id::{NodeSecret, PeerId};
use crate::net::link::NetInfo;
use crate::net::mem::MemNet;
use crate::net::service::{ServiceId, AUTH, EXIT, PORT};
use crate::net::stream::BoxFuture;
use crate::net::transport::{PeerConn, Transport};
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::mpsc;

/// Time limit of one test step: past it the test has failed, not hung.
pub(crate) const STEP: Duration = Duration::from_secs(5);

/// Node key from a seed.
pub(crate) fn secret(n: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[n; 32])
}

/// Node id from a seed.
pub(crate) fn pid(n: u8) -> PeerId {
    secret(n).peer_id()
}

/// An address hint by number: different numbers give different hints.
pub(crate) fn hint(n: u16) -> AddrHint {
    let ip = [10, 0, (n >> 8) as u8, n as u8];
    AddrHint::Ip(std::net::SocketAddr::from((ip, 4000)))
}

/// Address of node `pid(n)` with the hints `hint(..)` in order.
pub(crate) fn addr(n: u8, hints: &[u16]) -> PeerAddr {
    let mut addr = PeerAddr::new(pid(n));
    addr.hints = hints.iter().map(|h| hint(*h)).collect();
    addr
}

/// A test step with the limit `STEP`.
pub(crate) async fn within<T>(what: &str, fut: impl Future<Output = T>) -> T {
    match tokio::time::timeout(STEP, fut).await {
        Ok(value) => value,
        Err(_) => panic!("step \"{what}\" did not finish within {STEP:?}"),
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Shared journal of the order of events of several parts of one test.
#[derive(Clone, Default)]
pub(crate) struct Journal(Arc<Mutex<Vec<&'static str>>>);

impl Journal {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn push(&self, what: &'static str) {
        lock(&self.0).push(what);
    }

    pub(crate) fn entries(&self) -> Vec<&'static str> {
        lock(&self.0).clone()
    }
}

/// A marker that on destruction writes to the journal and/or raises a flag.
struct Mark {
    journal: Option<Journal>,
    what: &'static str,
    flag: Option<Arc<AtomicBool>>,
}

impl Drop for Mark {
    fn drop(&mut self) {
        if let Some(flag) = &self.flag {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(journal) = &self.journal {
            journal.push(self.what);
        }
    }
}

#[path = "test_support_script.rs"]
mod script;
pub(crate) use script::{ScriptReply, Script};

#[path = "test_support_spy.rs"]
mod spy;
pub(crate) use spy::Spy;

/// Probe service handler: hands every connection to the test and holds it until the task is cancelled.
pub(crate) struct Probe {
    tx: mpsc::UnboundedSender<Arc<dyn PeerConn>>,
    rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<Arc<dyn PeerConn>>>,
    calls: AtomicUsize,
    panic_first: bool,
    journal: Option<Journal>,
}

impl Probe {
    fn build(panic_first: bool, journal: Option<Journal>) -> Arc<Probe> {
        let (tx, rx) = mpsc::unbounded_channel();
        Arc::new(Probe {
            tx,
            rx: tokio::sync::Mutex::new(rx),
            calls: AtomicUsize::new(0),
            panic_first,
            journal,
        })
    }

    pub(crate) fn new() -> Arc<Probe> {
        Self::build(false, None)
    }

    /// The first call panics, the others work as usual.
    pub(crate) fn panicking_first() -> Arc<Probe> {
        Self::build(true, None)
    }

    /// Dropping the handler task writes "handler" to the journal.
    pub(crate) fn journaled(journal: &Journal) -> Arc<Probe> {
        Self::build(false, Some(journal.clone()))
    }

    /// How many times `handle` was called.
    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// The next connection that reached the handler (up to `STEP`).
    pub(crate) async fn next(&self) -> Arc<dyn PeerConn> {
        let got = within(
            "the connection reached the handler",
            async { self.rx.lock().await.recv().await },
        );
        got.await.expect("the Probe channel is closed")
    }

    /// A connection that has already arrived, if any.
    pub(crate) fn try_next(&self) -> Option<Arc<dyn PeerConn>> {
        self.rx.try_lock().ok()?.try_recv().ok()
    }
}

impl ServiceHandler for Probe {
    fn handle(&self, conn: Arc<dyn PeerConn>) -> BoxFuture<'static, ()> {
        let first = self.calls.fetch_add(1, Ordering::SeqCst) == 0;
        let boom = self.panic_first && first;
        let (tx, journal) = (self.tx.clone(), self.journal.clone());
        Box::pin(async move {
            let _mark = Mark {
                journal,
                what: "handler",
                flag: None,
            };
            assert!(!boom, "the Probe handler panicked on purpose");
            let _ = tx.send(conn.clone());
            std::future::pending::<()>().await;
        })
    }
}
