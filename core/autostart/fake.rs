//! Test double for [`AutostartBackend`]: a scripted state and call log.

use std::sync::Mutex;

use super::backend::{AutostartBackend, EntryState};

struct Inner {
    state: Result<EntryState, String>,
    enable: Result<(), String>,
    disable: Result<(), String>,
    calls: Vec<&'static str>,
}

/// Backend that answers from preset results and records which methods were called.
/// A successful `enable` moves the state to `Current`, a successful `disable` to `Absent`.
pub(crate) struct FakeAutostart {
    inner: Mutex<Inner>,
}

/// A fake in `state`, where `enable` and `disable` succeed.
pub(crate) fn leaked(state: EntryState) -> &'static FakeAutostart {
    leaked_with(Ok(state), Ok(()), Ok(()))
}

/// A fake with explicit results for the three methods.
pub(crate) fn leaked_with(
    state: Result<EntryState, String>,
    enable: Result<(), String>,
    disable: Result<(), String>,
) -> &'static FakeAutostart {
    let inner = Inner {
        state,
        enable,
        disable,
        calls: Vec::new(),
    };
    Box::leak(Box::new(FakeAutostart {
        inner: Mutex::new(inner),
    }))
}

impl FakeAutostart {
    /// Names of the methods called so far, in order: `"state"`, `"enable"`, `"disable"`.
    pub(crate) fn calls(&self) -> Vec<&'static str> {
        self.inner.lock().unwrap().calls.clone()
    }
}

impl AutostartBackend for FakeAutostart {
    fn state(&self) -> Result<EntryState, String> {
        let mut inner = self.inner.lock().unwrap();
        inner.calls.push("state");
        inner.state.clone()
    }

    fn enable(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        inner.calls.push("enable");
        if inner.enable.is_ok() {
            inner.state = Ok(EntryState::Current);
        }
        inner.enable.clone()
    }

    fn disable(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        inner.calls.push("disable");
        if inner.disable.is_ok() {
            inner.state = Ok(EntryState::Absent);
        }
        inner.disable.clone()
    }
}
