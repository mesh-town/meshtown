//! Receiving poller events in tests: `Seen`, `Probe` (waiting forward from a cursor), the clipboard stub.
//! What is not here: environment assembly and poller startup (`Rig` in `poller_test_support.rs`).

use super::*;

#[derive(Clone, Debug)]
pub(crate) enum Seen {
    View(View),
    Log(String),
    Exits(ExitMenu),
    Invite(InviteNote),
    Serve(ServeNote),
    StartFail(StartNote),
}

/// Clipboard stub: what was written and which refusal to return instead of writing.
#[derive(Default)]
pub(crate) struct ClipStub {
    pub(crate) copied: Vec<String>,
    pub(crate) fail: Option<&'static str>,
}

/// What the poller showed and printed; waiting goes forward from the cursor, for no longer than `WAIT`.
pub(crate) struct Probe {
    pub(crate) rx: mpsc::Receiver<Seen>,
    pub(crate) history: Vec<Seen>,
    pub(super) cursor: usize,
}

impl Probe {
    fn wait(&mut self, what: &str, pred: impl Fn(&Seen) -> bool) -> Seen {
        let deadline = std::time::Instant::now() + WAIT;
        loop {
            while self.cursor < self.history.len() {
                let item = self.history[self.cursor].clone();
                self.cursor += 1;
                if pred(&item) {
                    return item;
                }
            }
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(item) => self.history.push(item),
                Err(_) => panic!("timed out waiting for: {what}; got: {:?}", self.history),
            }
        }
    }

    pub(crate) fn view(&mut self, what: &str, pred: impl Fn(&View) -> bool) -> View {
        match self.wait(what, |s| matches!(s, Seen::View(v) if pred(v))) {
            Seen::View(v) => v,
            Seen::Log(_)
            | Seen::Exits(_)
            | Seen::Invite(_)
            | Seen::Serve(_)
            | Seen::StartFail(_) => unreachable!(),
        }
    }

    pub(crate) fn log(&mut self, needle: &str) -> String {
        match self.wait(needle, |s| matches!(s, Seen::Log(l) if l.contains(needle))) {
            Seen::Log(l) => l,
            Seen::View(_)
            | Seen::Exits(_)
            | Seen::Invite(_)
            | Seen::Serve(_)
            | Seen::StartFail(_) => unreachable!(),
        }
    }

    /// The exits submenu as seen by the poller: the next `Seen::Exits` for which `pred`
    /// holds, going forward from the cursor.
    pub(crate) fn exits(&mut self, what: &str, pred: impl Fn(&ExitMenu) -> bool) -> ExitMenu {
        match self.wait(what, |s| matches!(s, Seen::Exits(m) if pred(m))) {
            Seen::Exits(m) => m,
            Seen::View(_)
            | Seen::Log(_)
            | Seen::Invite(_)
            | Seen::Serve(_)
            | Seen::StartFail(_) => unreachable!(),
        }
    }

    /// The next invite item note for which `pred` holds, going forward from the cursor.
    pub(crate) fn invite(&mut self, what: &str, pred: impl Fn(&InviteNote) -> bool) -> InviteNote {
        match self.wait(what, |s| matches!(s, Seen::Invite(n) if pred(n))) {
            Seen::Invite(n) => n,
            Seen::View(_) | Seen::Log(_) | Seen::Exits(_) | Seen::Serve(_) | Seen::StartFail(_) => {
                unreachable!()
            }
        }
    }

    /// The next checkbox note for which `pred` holds, going forward from the cursor.
    pub(crate) fn serve(&mut self, what: &str, pred: impl Fn(&ServeNote) -> bool) -> ServeNote {
        match self.wait(what, |s| matches!(s, Seen::Serve(n) if pred(n))) {
            Seen::Serve(n) => n,
            Seen::View(_)
            | Seen::Log(_)
            | Seen::Exits(_)
            | Seen::Invite(_)
            | Seen::StartFail(_) => unreachable!(),
        }
    }

    /// The next node refusal reason for which `pred` holds, going forward from the cursor.
    pub(crate) fn start_note(
        &mut self,
        what: &str,
        pred: impl Fn(&StartNote) -> bool,
    ) -> StartNote {
        match self.wait(what, |s| matches!(s, Seen::StartFail(n) if pred(n))) {
            Seen::StartFail(n) => n,
            Seen::View(_) | Seen::Log(_) | Seen::Exits(_) | Seen::Invite(_) | Seen::Serve(_) => {
                unreachable!()
            }
        }
    }

    /// Everything that has arrived so far, without waiting.
    pub(crate) fn drain(&mut self) {
        while let Ok(item) = self.rx.try_recv() {
            self.history.push(item);
        }
    }

    pub(crate) fn logged(&mut self, needle: &str) -> bool {
        self.drain();
        self.history
            .iter()
            .any(|s| matches!(s, Seen::Log(l) if l.contains(needle)))
    }
}

/// Sender of poller events to the probe: wraps a value in `Seen`.
pub(super) fn sink<T: 'static>(
    tx: Mutex<mpsc::Sender<Seen>>,
    wrap: fn(T) -> Seen,
) -> Box<dyn Fn(T) + Send> {
    Box::new(move |n| {
        let _ = tx.lock().unwrap().send(wrap(n));
    })
}

/// Clipboard stub as a `CopyFn`: writes to `copied` or refuses per `fail`.
pub(super) fn clip_copy(clip: Arc<Mutex<ClipStub>>) -> CopyFn {
    Arc::new(move |text: &str| {
        let mut stub = clip.lock().unwrap();
        match stub.fail {
            Some(reason) => Err(reason),
            None => {
                stub.copied.push(text.to_string());
                Ok(())
            }
        }
    })
}
