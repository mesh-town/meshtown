//! Tasks of the exit streams of one connection: they end together with it.

use tokio::task::AbortHandle;

/// Abort handles of the stream tasks of one connection; dropping the set aborts every task still running.
#[derive(Default)]
pub(super) struct StreamTasks(Vec<AbortHandle>);

impl StreamTasks {
    /// Remembers a task; the finished ones are forgotten first so the list doesn't grow with the stream count.
    pub(super) fn push(&mut self, handle: AbortHandle) {
        self.0.retain(|h| !h.is_finished());
        self.0.push(handle);
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
}

impl Drop for StreamTasks {
    fn drop(&mut self) {
        self.0.iter().for_each(AbortHandle::abort);
    }
}

#[cfg(test)]
#[path = "stream_tasks_tests.rs"]
mod tests;
