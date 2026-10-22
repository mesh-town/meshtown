//! The SOCKS5 accept task handle: `close` returns once the listener is already closed.

use std::sync::{Arc, Mutex};

use tokio::task::JoinHandle;

/// The SOCKS5 accept task in a shared cell. The port must be free by the time
/// `RunningNode::shutdown` returns, because the next node is started on it right away; `abort()` alone does not
/// close the socket, destroying the future closes it — and `close` waits for that.
#[derive(Clone, Default)]
pub(crate) struct SocksTask(Arc<Mutex<Option<JoinHandle<()>>>>);

impl SocksTask {
    pub(crate) fn new(handle: JoinHandle<()>) -> SocksTask {
        SocksTask(Arc::new(Mutex::new(Some(handle))))
    }

    /// Aborts the task and waits for it to end; an empty handle (SOCKS5 is off or already closed) —
    /// returns at once. Swallows the task's cancellation and panic.
    pub(crate) async fn close(&self) {
        let taken = self.0.lock().unwrap_or_else(|e| e.into_inner()).take();
        let Some(handle) = taken else { return };
        handle.abort();
        let _ = handle.await;
    }
}

#[cfg(test)]
#[path = "socks_task_tests.rs"]
pub(in crate::node) mod tests;
