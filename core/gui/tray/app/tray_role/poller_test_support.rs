//! Shared rig for poller tests: a fake environment and event reception (`Rig`, `Probe`), timeouts and view predicates.

use super::*;

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::super::views::ABSENT_VIEW;
use crate::gui::tray::app::exit_menu::ExitMenu;
use crate::gui::tray::app::proxy_state::ProxyState;
use crate::gui::tray::app::start_note::StartNote;
use crate::gui::tray::app::tray_actions::{InviteNote, ServeNote};

pub(crate) const T: PollTiming = PollTiming {
    poll_interval: Duration::from_millis(100),
    own_unavailable_after: Duration::from_secs(1),
    // A refused loopback connect takes 2–2.5 s on Windows, so the stop deadline there has to cover one.
    stop_wait: Duration::from_secs(if cfg!(windows) { 5 } else { 1 }),
    foreign_gone_after_failures: 3,
    note_visible_for: Duration::from_millis(400),
};
#[path = "poller_test_probe.rs"]
mod probe;
pub(crate) use probe::*;

/// How long a test waits for a poller event. On Windows a connect to a closed loopback port is not refused at
/// once but fails after the ~2 s connect timeout, so "node stopped" (three failed polls in a row) needs ~6 s.
pub(crate) const WAIT: Duration = Duration::from_secs(if cfg!(windows) { 15 } else { 5 });

/// Default proxy address "from the settings" in tests.
pub(crate) fn test_proxy() -> ProxyState {
    ProxyState::On("127.0.0.1:1080".to_string())
}

pub(crate) type Spawn = Box<dyn FnMut() -> std::io::Result<Child> + Send>;

pub(crate) fn spawn_panics() -> Spawn {
    Box::new(|| panic!("spawn must not be called"))
}

/// Stub for the proxy port check and the node log tail (the real thing is `run`): what to return now.
#[derive(Clone, Default)]
pub(crate) struct StartStub {
    busy: Arc<Mutex<Option<u16>>>,
    log: Arc<Mutex<Option<String>>>,
}

impl StartStub {
    pub(crate) fn set_busy(&self, port: Option<u16>) {
        *self.busy.lock().unwrap() = port;
    }

    #[cfg(unix)]
    pub(crate) fn set_log(&self, text: Option<&str>) {
        *self.log.lock().unwrap() = text.map(str::to_string);
    }
}

/// Test children: pids go into a shared list; `Drop` finishes everyone off with `kill -9` (even if a test panics).
#[cfg(unix)]
#[derive(Clone, Default)]
pub(crate) struct Kids(pub(crate) Arc<Mutex<Vec<u32>>>);

#[cfg(unix)]
impl Kids {
    pub(crate) fn pids(&self) -> Vec<u32> {
        self.0.lock().unwrap().clone()
    }
}

#[cfg(unix)]
pub(crate) struct Reaper(pub(crate) Kids);

#[cfg(unix)]
impl Drop for Reaper {
    fn drop(&mut self) {
        for pid in self.0.pids() {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status();
        }
    }
}

#[cfg(unix)]
pub(crate) fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Runs `program args…` and records the pid in `kids`.
#[cfg(unix)]
pub(crate) fn spawn_cmd(
    kids: &Kids,
    program: &'static str,
    args: &'static [&'static str],
) -> Spawn {
    let kids = kids.clone();
    Box::new(move || {
        let child = std::process::Command::new(program).args(args).spawn()?;
        kids.0.lock().unwrap().push(child.id());
        Ok(child)
    })
}

/// A port nobody is listening on right now (the connection is rejected).
pub(crate) async fn refused_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    l.local_addr().unwrap().port()
}

/// `StartFail` on the port and log stubs; refusal reasons go to the probe.
fn stub_start_fail(stub: StartStub, ev_tx: mpsc::Sender<Seen>) -> StartFail {
    let (busy, node_log) = (stub.busy, stub.log);
    StartFail::new(
        Box::new(move || *busy.lock().unwrap()),
        Box::new(move || node_log.lock().unwrap().clone()),
        sink(Mutex::new(ev_tx), Seen::StartFail),
    )
}

/// A wait that completes when the test sends a value (or drops the sender).
fn waiter(rx: oneshot::Receiver<()>) -> Once {
    Box::pin(async move {
        let _ = rx.await;
    })
}

pub(crate) struct Rig {
    pub(crate) probe: Probe,
    pub(crate) clip: Arc<Mutex<ClipStub>>,
    cmds: tokio::sync::mpsc::UnboundedSender<NodeCmd>,
    quit: Option<oneshot::Sender<()>>,
    signal: Option<oneshot::Sender<()>>,
    pub(crate) task: tokio::task::JoinHandle<i32>,
}

impl Rig {
    pub(crate) fn start(port: u16, token_path: PathBuf, web_enabled: bool, spawn: Spawn) -> Rig {
        Rig::start_with_proxy(port, token_path, web_enabled, spawn, test_proxy())
    }

    /// The same, but with stubs for the proxy port check and the node log.
    pub(crate) fn start_with(
        port: u16,
        token_path: PathBuf,
        web_enabled: bool,
        spawn: Spawn,
        stub: StartStub,
    ) -> Rig {
        Rig::start_full(port, token_path, web_enabled, spawn, test_proxy(), stub)
    }

    /// The same, but with a proxy address from the "settings" (`Off` — the proxy is off).
    pub(crate) fn start_with_proxy(
        port: u16,
        token_path: PathBuf,
        web_enabled: bool,
        spawn: Spawn,
        proxy: ProxyState,
    ) -> Rig {
        Rig::start_full(
            port,
            token_path,
            web_enabled,
            spawn,
            proxy,
            StartStub::default(),
        )
    }

    /// The common builder: the whole poller environment, including the `StartFail` stubs.
    pub(crate) fn start_full(
        port: u16,
        token_path: PathBuf,
        web_enabled: bool,
        spawn: Spawn,
        proxy: ProxyState,
        stub: StartStub,
    ) -> Rig {
        let (ev_tx, ev_rx) = mpsc::channel::<Seen>();
        let clip = Arc::new(Mutex::new(ClipStub::default()));
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (quit_tx, quit_rx) = oneshot::channel::<()>();
        let (signal_tx, signal_rx) = oneshot::channel::<()>();
        let env = PollEnv {
            port,
            token_path,
            web_enabled,
            proxy,
            spawn,
            send_view: sink(Mutex::new(ev_tx.clone()), Seen::View),
            send_exit_menu: sink(Mutex::new(ev_tx.clone()), Seen::Exits),
            log: sink(Mutex::new(ev_tx.clone()), Seen::Log),
            copy_to_clipboard_fn: clip_copy(clip.clone()),
            send_invite_note: sink(Mutex::new(ev_tx.clone()), Seen::Invite),
            send_serve_note: sink(Mutex::new(ev_tx.clone()), Seen::Serve),
            start_fail: stub_start_fail(stub, ev_tx),
            cmds: cmd_rx,
            quit: waiter(quit_rx),
            signal: waiter(signal_rx),
        };
        let task =
            crate::node::output::spawn_on(&tokio::runtime::Handle::current(), poll_node(env, T));
        Rig {
            probe: Probe {
                rx: ev_rx,
                history: Vec::new(),
                cursor: 0,
            },
            clip,
            cmds: cmd_tx,
            quit: Some(quit_tx),
            signal: Some(signal_tx),
            task,
        }
    }

    pub(crate) fn cmd(&self, c: NodeCmd) {
        self.cmds.send(c).expect("the poller is alive");
    }

    pub(crate) fn quit(&mut self) {
        let _ = self.quit.take().unwrap().send(());
    }

    pub(crate) fn signal(&mut self) {
        let _ = self.signal.take().unwrap().send(());
    }

    pub(crate) async fn code(self) -> i32 {
        let Rig {
            task, quit, signal, ..
        } = self;
        let code = tokio::time::timeout(WAIT, task)
            .await
            .expect("the poller returned a code in time")
            .expect("the poller did not panic");
        drop((quit, signal));
        code
    }
}

pub(crate) fn is_absent(v: &View) -> bool {
    *v == ABSENT_VIEW
}

pub(crate) fn is_ready(v: &View) -> bool {
    v.page == PageState::Ready && v.node == NodeState::Running
}
