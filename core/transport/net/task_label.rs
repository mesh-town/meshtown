//! Network label of the current task and spawn wrappers that carry it into new tasks.
//!
//! What it does: keeps the label in a task-local; `spawn*` copy it into the spawned task or closure.
//! What it doesn't do: no logging; the log reads the label through `current_net`.
//! Links: re-exported by `node::output`; used by every module that spawns tasks.

use std::future::Future;
use std::sync::Arc;

use tokio::runtime::Handle;
use tokio::task::{JoinHandle, JoinSet};

tokio::task_local! {
    static NET: Option<Arc<str>>;
}

/// Runs `fut` with the network label `label`: log lines written inside carry it.
pub fn with_net<F: Future>(label: Option<Arc<str>>, fut: F) -> impl Future<Output = F::Output> {
    NET.scope(label, fut)
}

/// The network label of the current task; `None` outside a labeled task.
pub fn current_net() -> Option<Arc<str>> {
    NET.try_with(Clone::clone).ok().flatten()
}

/// `tokio::spawn` that carries the network label into the new task.
#[allow(clippy::disallowed_methods)]
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(with_net(current_net(), fut))
}

/// `Handle::spawn` that carries the network label into the new task.
#[allow(clippy::disallowed_methods)]
pub fn spawn_on<F>(handle: &Handle, fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle.spawn(with_net(current_net(), fut))
}

/// `JoinSet::spawn` that carries the network label into the new task.
#[allow(clippy::disallowed_methods)]
pub fn spawn_in<T, F>(set: &mut JoinSet<T>, fut: F) -> tokio::task::AbortHandle
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    set.spawn(with_net(current_net(), fut))
}

/// `spawn_blocking` that carries the network label into the closure.
#[allow(clippy::disallowed_methods)]
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let label = current_net();
    tokio::task::spawn_blocking(move || NET.sync_scope(label, f))
}

#[cfg(test)]
#[path = "task_label_tests.rs"]
mod tests;
