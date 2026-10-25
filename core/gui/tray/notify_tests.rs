//! Tests of `notify.rs`: the worker thread that shows a notification off the caller's thread, and the
//! runner of the platform's notification program.

use super::*;
use crate::gui::tray::Core;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const NOTICE: Notice = Notice {
    title: "Title of the test",
    body: "Body of the test",
};

mod spawn_notify {
    use super::*;

    #[test]
    fn spawn_notify_passes_title_and_body() {
        let handle = spawn_notify(&REMEMBERING, NOTICE).expect("the thread starts");

        handle.join().expect("the worker does not panic");

        let seen = REMEMBERED.lock().unwrap().clone();
        assert_eq!(
            seen,
            vec![(NOTICE.title.to_string(), NOTICE.body.to_string())]
        );
    }

    #[test]
    fn spawn_notify_logs_refusal() {
        let handle = spawn_notify(&REFUSING, NOTICE).expect("the thread starts");

        let joined = handle.join();

        assert!(joined.is_ok(), "a refusal must not panic the worker");
        let text = journal_text();
        assert!(
            text.contains("invite: notification not shown — refused-by-test-stub"),
            "journal: {text}"
        );
    }

    #[test]
    fn spawn_notify_does_not_wait() {
        let (release, gate) = channel();
        *GATE.lock().unwrap() = Some(gate);

        let handle = spawn_notify(&WAITING, NOTICE).expect("the thread starts");

        assert!(
            !WAITING_DONE.load(Ordering::SeqCst),
            "spawn_notify returned only after notify finished"
        );
        release.send(()).unwrap();
        handle.join().expect("the worker does not panic");
        assert!(WAITING_DONE.load(Ordering::SeqCst), "notify never finished");
    }
}

mod run_notifier {
    use super::*;

    #[test]
    fn missing_program_is_no_tool() {
        let result = run_notifier("meshtown-no-such-tool", &[], &[]);

        assert_eq!(result, Err(REASON_NOTIFY_NO_TOOL));
    }

    #[cfg(unix)]
    #[test]
    fn zero_exit_is_ok() {
        assert_eq!(run_notifier("true", &[], &[]), Ok(()));
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_exit_is_failed() {
        let result = run_notifier("false", &[], &[]);

        assert_eq!(result, Err(REASON_NOTIFY_FAILED));
    }

    #[cfg(unix)]
    #[test]
    fn hung_notifier_is_killed_after_the_limit() {
        let start = Instant::now();

        let result = run_notifier_within("sleep", &["5"], &[], Duration::from_millis(200));

        assert_eq!(result, Err(REASON_NOTIFY_FAILED));
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
    }

    #[cfg(unix)]
    #[test]
    fn notifier_success_is_ok() {
        let result = run_notifier_within("true", &[], &[], Duration::from_secs(5));

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn missing_notifier_is_no_tool() {
        let result = run_notifier_within("meshtown-no-such-tool", &[], &[], Duration::from_secs(1));

        assert_eq!(result, Err(REASON_NOTIFY_NO_TOOL));
    }

    #[cfg(unix)]
    #[test]
    fn failing_notifier_is_failed() {
        let result = run_notifier_within("false", &[], &[], Duration::from_secs(5));

        assert_eq!(result, Err(REASON_NOTIFY_FAILED));
    }

    #[cfg(unix)]
    #[test]
    fn env_reaches_program() {
        let args = ["-c", "test \"$MT_X\" = y"];

        let result = run_notifier("sh", &args, &[("MT_X", "y")]);

        assert_eq!(result, Ok(()));
    }
}

// ---- stub backends and helpers ----

fn journal_text() -> String {
    format!("{:?}", crate::node::output::log_page(0, 100_000).lines)
}

/// A backend that remembers what it was asked to show.
struct RememberingTray;
static REMEMBERING: RememberingTray = RememberingTray;
static REMEMBERED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// A backend whose `notify` refuses.
struct RefusingTray;
static REFUSING: RefusingTray = RefusingTray;

/// A backend whose `notify` waits for a signal from the test (at most 5 s) and only then marks itself done.
struct WaitingTray;
static WAITING: WaitingTray = WaitingTray;
static GATE: Mutex<Option<Receiver<()>>> = Mutex::new(None);
static WAITING_DONE: AtomicBool = AtomicBool::new(false);

macro_rules! no_event_loop {
    () => {
        fn run(&self, _core: Core) -> ! {
            unreachable!("tests never start the event loop")
        }
        fn wake(&self) {}
        fn open_url(&self, _url: &str) -> Result<(), String> {
            Ok(())
        }
        fn copy_to_clipboard(&self, _text: &str) -> Result<(), &'static str> {
            Ok(())
        }
    };
}

impl TrayBackend for RememberingTray {
    no_event_loop!();
    fn notify(&self, title: &str, body: &str) -> Result<(), &'static str> {
        REMEMBERED
            .lock()
            .unwrap()
            .push((title.to_string(), body.to_string()));
        Ok(())
    }
}

impl TrayBackend for RefusingTray {
    no_event_loop!();
    fn notify(&self, _title: &str, _body: &str) -> Result<(), &'static str> {
        Err("refused-by-test-stub")
    }
}

impl TrayBackend for WaitingTray {
    no_event_loop!();
    fn notify(&self, _title: &str, _body: &str) -> Result<(), &'static str> {
        let gate = GATE.lock().unwrap().take().expect("the test set the gate");
        let _ = gate.recv_timeout(Duration::from_secs(5));
        WAITING_DONE.store(true, Ordering::SeqCst);
        Ok(())
    }
}
