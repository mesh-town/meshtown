//! An exclusive advisory lock on a file, held until the value is dropped. Used to serialize
//! writers of the settings file (and parallel `join` commands) across threads and processes.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

const POLL_PAUSE: Duration = Duration::from_millis(5);

/// Holds the lock while alive; the lock is released when the file is closed on drop.
#[derive(Debug)]
pub(crate) struct FileLock {
    file: File,
}

impl FileLock {
    /// Opens (creating if needed) `path` and takes an exclusive lock, polling for up to `wait`.
    /// Still busy after `wait` — `Err(TimedOut)`. A platform without file locks — `Ok(None)`:
    /// the caller works without a lock.
    pub(crate) fn exclusive(path: &Path, wait: Duration) -> io::Result<Option<FileLock>> {
        let file = open_lock_file(path)?;
        let deadline = Instant::now() + wait;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Some(FileLock { file })),
                Err(TryLockError::Error(e)) if e.kind() == io::ErrorKind::Unsupported => {
                    return Ok(None)
                }
                Err(TryLockError::Error(e)) => return Err(e),
                Err(TryLockError::WouldBlock) => pause_or_time_out(deadline)?,
            }
        }
    }
}

fn pause_or_time_out(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the lock is held by another process",
        ));
    }
    std::thread::sleep(POLL_PAUSE);
    Ok(())
}

fn open_lock_file(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
#[path = "file_lock_tests.rs"]
mod tests;
