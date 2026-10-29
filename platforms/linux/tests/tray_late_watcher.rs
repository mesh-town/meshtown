//! End-to-end check of the tray icon raise against a StatusNotifierWatcher that appears late.
//!
//! The test needs a session bus, so it runs only with `MESHTOWN_TRAY_E2E=1` (see `tray-e2e.sh`, which
//! starts a private bus); otherwise it prints a note and passes. It uses the retry loop of the core
//! (`raise_with_retry`) on top of a host that builds the real `tray_icon` icon, and a fake watcher that
//! takes its bus name only after a delay.

#![cfg(target_os = "linux")]

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use meshtown_core::gui::tray::{raise_with_retry, IconHost, Raise, RetryPlan};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use zbus::blocking::Connection;

const WATCHER_NAME: &str = "org.kde.StatusNotifierWatcher";
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
const WATCHER_DELAY: Duration = Duration::from_millis(2500);
const REGISTRATION_DEADLINE: Duration = Duration::from_secs(5);

#[test]
fn tray_registers_when_watcher_appears_late() {
    if std::env::var("MESHTOWN_TRAY_E2E").as_deref() != Ok("1") {
        eprintln!("skipped: set MESHTOWN_TRAY_E2E=1 and run under a session bus (see tray-e2e.sh)");
        return;
    }
    // given: a watcher that takes its name only after a delay
    let (registered_tx, registered_rx) = mpsc::channel();
    let started = Instant::now();
    let watcher = thread::spawn(move || {
        thread::sleep(WATCHER_DELAY);
        start_watcher(registered_tx)
    });
    let mut host = E2eHost::default();

    // when: one attempt, as the loop worked before retrying
    let single = raise_with_retry(&mut host, plan(Duration::from_millis(300), 1));

    // then: the defect is reproduced, the watcher was really absent
    assert!(
        started.elapsed() < WATCHER_DELAY,
        "phase A ran past the watcher start"
    );
    assert_eq!(
        single,
        Raise::GaveUp { attempts: 1 },
        "logs: {:?}",
        host.logs
    );
    assert!(
        registered_rx.try_recv().is_err(),
        "nothing may be registered yet"
    );

    // when: the retrying loop
    let retried = raise_with_retry(&mut host, plan(Duration::from_millis(300), 20));

    // then: the icon is raised after the watcher appears and registers itself
    assert!(
        matches!(retried, Raise::Attached { attempts } if attempts >= 2),
        "outcome: {retried:?}, logs: {:?}",
        host.logs
    );
    let item = wait_registration(&registered_rx);
    assert!(!item.is_empty(), "the registered item name is empty");
    let _watcher_connection = watcher.join().expect("the watcher thread panicked");
    drop(host.icon.take());
}

/// A retry plan with the pauses of the late-watcher check: `first` doubled up to twice as long.
fn plan(first: Duration, max_attempts: u32) -> RetryPlan {
    RetryPlan {
        first,
        cap: first * 2,
        max_attempts,
    }
}

/// Waits for the watcher to receive a registration and returns the registered name.
fn wait_registration(registered: &Receiver<String>) -> String {
    registered
        .recv_timeout(REGISTRATION_DEADLINE)
        .expect("the icon did not register within the deadline")
}

/// The fake StatusNotifierWatcher: reports every registered item to the test.
struct FakeWatcher {
    registered: Mutex<Sender<String>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl FakeWatcher {
    fn register_status_notifier_item(&self, service: &str) {
        let _ = self.registered.lock().unwrap().send(service.to_string());
    }

    fn register_status_notifier_host(&self, _service: &str) {}

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Takes the watcher name on the session bus; the returned connection keeps it while it lives.
fn start_watcher(registered: Sender<String>) -> Connection {
    let watcher = FakeWatcher {
        registered: Mutex::new(registered),
    };
    zbus::blocking::connection::Builder::session()
        .expect("session bus address")
        .name(WATCHER_NAME)
        .expect("watcher bus name")
        .serve_at(WATCHER_PATH, watcher)
        .expect("watcher object")
        .build()
        .expect("watcher connection")
}

/// Builds the real icon like `Core::attach_ui` does, sleeps for pauses and never asks to quit.
#[derive(Default)]
struct E2eHost {
    icon: Option<TrayIcon>,
    logs: Vec<String>,
}

impl IconHost for E2eHost {
    fn try_attach(&mut self) -> Result<(), String> {
        let icon = Icon::from_rgba(vec![255; 16 * 16 * 4], 16, 16).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;
        self.icon = Some(tray);
        Ok(())
    }

    fn wait(&mut self, pause: Duration) -> Option<i32> {
        thread::sleep(pause);
        None
    }

    fn log(&mut self, text: &str) {
        eprintln!("[host] {text}");
        self.logs.push(text.to_string());
    }
}
