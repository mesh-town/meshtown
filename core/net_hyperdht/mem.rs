//! In-memory record store for tests, with the townjump rules:
//! a `seq` not greater than the stored one is silently not written, `put` is still `Ok`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use super::store::{Record, RecordKey, RecordStore};
use crate::net::{BoxFuture, NetError, NetResult};

#[derive(Default)]
pub(crate) struct MemRecords {
    rec: Mutex<Option<Record>>,
    fail: AtomicUsize,
    puts: AtomicUsize,
    stalled: AtomicBool,
}

impl MemRecords {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The next `n` calls (get or put) return an error.
    pub(crate) fn fail_next(&self, n: usize) {
        self.fail.store(n, Ordering::SeqCst);
    }

    /// Number of `put` calls (including silently dropped ones).
    pub(crate) fn puts(&self) -> usize {
        self.puts.load(Ordering::SeqCst)
    }

    /// Calls hang (to check the call deadline).
    pub(crate) fn stall(&self, on: bool) {
        self.stalled.store(on, Ordering::SeqCst);
    }

    #[allow(deprecated)] // `try_update` needs Rust 1.95, above the MSRV (1.94)
    fn take_failure(&self) -> bool {
        self.fail
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
    }

    async fn gate(&self) -> NetResult<()> {
        while self.stalled.load(Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        match self.take_failure() {
            true => Err(NetError::Other(anyhow::anyhow!("MemRecords: injected failure"))),
            false => Ok(()),
        }
    }
}

impl RecordStore for MemRecords {
    fn get(&self, _key: &RecordKey) -> BoxFuture<'_, NetResult<Option<Record>>> {
        Box::pin(async move {
            self.gate().await?;
            Ok(self.rec.lock().expect("MemRecords lock").clone())
        })
    }

    fn put(&self, _key: &RecordKey, value: Vec<u8>, seq: u64) -> BoxFuture<'_, NetResult<()>> {
        Box::pin(async move {
            self.puts.fetch_add(1, Ordering::SeqCst);
            self.gate().await?;
            let mut slot = self.rec.lock().expect("MemRecords lock");
            if slot.as_ref().is_none_or(|r| seq > r.seq) {
                *slot = Some(Record { seq, value });
            }
            Ok(())
        })
    }
}
