//! Tests of the process host: the order "log capacity → page → body → page shutdown",
//! the page roles and the error texts. The network is 127.0.0.1 only.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use std::path::PathBuf;

use super::{Host, HostSlots, PageRole, SlotContext};
use crate::config::config_file::{
    self, net_slot_path, new_slot_id, save_net_file, CliOverrides, ConfigSource, ListenAccess,
    NetSettings, NetSlotId, PathRule, RunConfig, TransportMode,
};
use crate::members::network_key::NetworkKey;
use crate::node::guard_sources::GuardSources;
use crate::node::node_ctl::{NodeCtl, Relaunch};
use crate::node::output::{log_page, log_summary, say, set_log_capacity, with_net};
use crate::proxy::{BindError, ProxyFront, ProxySeat};
use crate::transport::net::NodeSecret;
use crate::web_gui::{ConfigState, HostControl};

/// Log capacity in tests: larger than the default (2000), so other lines are not cut.
const TEST_RING: u64 = 2001;

/// A launch config without a file: the page on `port`, access only from this computer.
fn run_config(port: u16) -> RunConfig {
    let cli = CliOverrides {
        web_port: Some(port),
        ..Default::default()
    };
    let mut run = config_file::prepare(&ConfigSource::Disabled, &cli, false, 0).unwrap();
    run.effective_settings.process.web_access = ListenAccess::Localhost;
    run.effective_settings.process.log_ring_lines = TEST_RING;
    // Each run owns the process proxy port: tests of one binary run side by side.
    run.effective_settings.process.socks_port = 0;
    run
}

/// A free port: take it and release it at once.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    l.local_addr().unwrap().port()
}

fn ctl_of(run: &RunConfig) -> Arc<NodeCtl> {
    NodeCtl::new(&run.effective_settings)
}

/// All log lines that appeared after `seq`, as one text.
fn lines_after(seq: u64) -> String {
    let page = log_page(seq, 5000);
    let texts: Vec<String> = page.lines.into_iter().map(|l| l.text).collect();
    texts.join("\n")
}

async fn port_open(port: u16) -> bool {
    tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
}

#[tokio::test]
async fn disabled_page_runs_body_without_line() {
    let mut run = run_config(free_port());
    run.effective_settings.process.web_enabled = false;
    let before = log_summary(0).last_seq;
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    let ctl = ctl_of(&run);
    let res = Host::run_with_web_page(run, ctl, PageRole::Optional, |_, _| async move {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await;
    assert!(res.is_ok());
    assert!(ran.load(Ordering::SeqCst));
    assert!(!lines_after(before).contains("the page is turned off"));
}

#[tokio::test]
async fn body_runs_between_page_up_and_page_down() {
    let port = free_port();
    let run = run_config(port);
    let before = log_summary(0).last_seq;
    let order: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let inner = order.clone();
    let ctl = ctl_of(&run);
    let res = Host::run_with_web_page(run, ctl, PageRole::Optional, move |_, _| async move {
        assert_eq!(log_summary(0).ring_lines, TEST_RING);
        assert!(port_open(port).await, "the page must be up in the body");
        say("host-test", format!("body {port}"));
        inner.lock().unwrap().push("body");
        Ok(())
    })
    .await;
    assert!(res.is_ok());
    assert_eq!(*order.lock().unwrap(), vec!["body"]);
    assert!(!port_open(port).await, "after serve the page is stopped");
    let page = log_page(before, 5000);
    let seq_of = |needle: &str| {
        page.lines
            .iter()
            .find(|l| l.text.contains(needle))
            .map(|l| l.seq)
    };
    let up = seq_of(&format!("127.0.0.1:{port}")).expect("page start line");
    let body = seq_of(&format!("body {port}")).expect("body line");
    assert!(
        up < body,
        "the page announcement comes before the body line"
    );
    set_log_capacity(2000);
}

#[tokio::test]
async fn busy_port_gives_fallback_line_and_runs_body() {
    let busy = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = busy.local_addr().unwrap().port();
    let run = run_config(port);
    let before = log_summary(0).last_seq;
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    let ctl = ctl_of(&run);
    let res = Host::run_with_web_page(run, ctl, PageRole::Optional, |_, _| async move {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await;
    assert!(res.is_ok());
    assert!(ran.load(Ordering::SeqCst));
    assert!(lines_after(before).contains(&format!("port {port} is in use")));
    drop(busy);
}

#[tokio::test]
async fn body_error_is_returned_and_page_is_stopped() {
    let port = free_port();
    let run = run_config(port);
    let ctl = ctl_of(&run);
    let res = Host::run_with_web_page(run, ctl, PageRole::Optional, |_, _| async move {
        Err(anyhow::anyhow!("body broke"))
    })
    .await;
    assert_eq!(res.unwrap_err().to_string(), "body broke");
    assert!(!port_open(port).await);
}

#[tokio::test]
async fn entry_role_fails_without_page() {
    let busy = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = busy.local_addr().unwrap().port();
    let mut cases = vec![(run_config(port), "--web-port")];
    let mut off = run_config(free_port());
    off.effective_settings.process.web_enabled = false;
    cases.push((off, "web.enabled"));
    for (run, advice) in cases {
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        let ctl = ctl_of(&run);
        let res = Host::run_with_web_page(run, ctl, PageRole::OnlyEntry, |_, _| async move {
            flag.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await;
        let text = res.unwrap_err().to_string();
        assert!(text.contains("the node page is unavailable"), "{text}");
        assert!(text.contains(advice), "{text}");
        assert!(text.contains("meshtown run --invite"), "{text}");
        assert!(!ran.load(Ordering::SeqCst));
    }
    drop(busy);
}

#[tokio::test]
async fn entry_role_runs_body_when_page_is_up() {
    let run = run_config(free_port());
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    let ctl = ctl_of(&run);
    let res = Host::run_with_web_page(run, ctl, PageRole::OnlyEntry, |_, _| async move {
        flag.store(true, Ordering::SeqCst);
        Ok(())
    })
    .await;
    assert!(res.is_ok());
    assert!(ran.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_gives_the_controller_the_guard_sources() {
    use crate::members::network_key::NetworkKey;
    use crate::node::{start::start_node, NodeConfig};
    let mut run = run_config(0);
    run.effective_settings.process.node_memory_warn_mib = 1_000_000;
    run.effective_settings.process.node_memory_exit_mib = 0;
    let ctl = ctl_of(&run);
    let res = Host::run_with_web_page(run, ctl, PageRole::Optional, |_, ctl| async move {
        let sources = ctl.guard_sources().expect("the host must set the sources");
        assert!(sources.snapshot().is_empty(), "no node yet");
        let node = start_node(NodeConfig::for_loopback_test(NetworkKey::generate()?)).await?;
        ctl.attach(&node);
        assert_eq!(sources.snapshot().len(), 1, "attach must fill the sources");
        node.shutdown().await;
        ctl.release_guard_sources();
        assert!(
            sources.snapshot().is_empty(),
            "release must clear the sources"
        );
        Ok(())
    })
    .await;
    assert!(res.is_ok(), "{res:?}");
}

#[tokio::test]
async fn panicked_body_is_an_error() {
    let run = run_config(free_port());
    let ctl = ctl_of(&run);
    let res = Host::run_with_web_page(run, ctl, PageRole::Optional, |_, _| async move {
        panic!("body panicked");
        #[allow(unreachable_code)]
        Ok(())
    })
    .await;
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("node task of slot main"));
}

#[tokio::test]
async fn page_is_up_until_the_body_ends() {
    let port = free_port();
    let run = run_config(port);
    let ctl = ctl_of(&run);
    let seen = Arc::new(AtomicBool::new(false));
    let flag = seen.clone();
    let res = Host::run_with_web_page(run, ctl, PageRole::Optional, move |_, _| async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        flag.store(port_open(port).await, Ordering::SeqCst);
        Ok(())
    })
    .await;
    assert!(res.is_ok());
    assert!(
        seen.load(Ordering::SeqCst),
        "the page outlives the slot task"
    );
    assert!(!port_open(port).await, "and goes down after it");
}

#[tokio::test]
async fn slot_added_while_waiting_is_waited_for() {
    use crate::config::config_file::NetSlotId;
    use crate::web_gui::{ConfigState, HostControl, SlotHandle};

    let run = run_config(free_port());
    let main_ctl = ctl_of(&run);
    let host = super::HostSlots::with_main(main_ctl.clone(), ConfigState::from_run(&run));
    let waiting = main_ctl.clone();
    host.set_task(
        &NetSlotId::main(),
        crate::node::output::spawn(async move {
            waiting.next_command().await;
            Ok(())
        }),
    );
    let waiter = crate::node::output::spawn({
        let host = host.clone();
        async move { Host::wait_slots(&host).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let late_ctl = ctl_of(&run);
    let handle = SlotHandle {
        id: NetSlotId::parse("ab2c3d").unwrap(),
        node: late_ctl.clone(),
        config: Arc::new(Mutex::new(ConfigState::from_run(&run))),
    };
    let finished = Arc::new(AtomicBool::new(false));
    let flag = finished.clone();
    let waiting = late_ctl.clone();
    host.push_slot(handle, late_ctl, async move {
        waiting.next_command().await;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        flag.store(true, Ordering::SeqCst);
        Ok(())
    })
    .unwrap();
    host.stop_all();
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10), waiter).await;
    assert!(outcome.expect("the wait ends").unwrap().is_ok());
    assert!(
        finished.load(Ordering::SeqCst),
        "the late slot was waited for"
    );
}

mod process_proxy_off {
    use super::*;

    #[tokio::test]
    async fn flag_is_named_in_the_line() {
        // given: the proxy turned off by a flag
        let mut run = run_config(free_port());
        run.effective_settings.process.socks_enabled = false;
        run.cli_override_paths.push("socks.enabled");
        let mark = log_summary(0).last_seq;
        // when: the host runs
        let res = run_labeled("proxy-off-flag", run, PageRole::Optional).await;
        // then: the line says "by the flag"
        assert!(res.is_ok(), "{res:?}");
        let lines = socks_lines("proxy-off-flag", mark);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("off by the --no-socks flag"), "{lines:?}");
    }

    #[tokio::test]
    async fn setting_is_named_in_the_line() {
        // given: the proxy turned off in the settings file, no flag
        let mut run = run_config(free_port());
        run.effective_settings.process.socks_enabled = false;
        let mark = log_summary(0).last_seq;
        // when: the host runs
        let res = run_labeled("proxy-off-file", run, PageRole::Optional).await;
        // then: the line says "in settings"
        assert!(res.is_ok(), "{res:?}");
        let lines = socks_lines("proxy-off-file", mark);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("off in settings"), "{lines:?}");
    }

    #[tokio::test]
    async fn takes_no_port_and_gives_the_slot_no_listener() {
        // given: the proxy off on a port that is free
        let port = free_port();
        let mut run = run_config(free_port());
        run.effective_settings.process.socks_enabled = false;
        run.effective_settings.process.socks_port = port;
        // when: the body looks at the port and at the face
        let res = run_with("proxy-off-port", run, PageRole::Optional, move |_, ctl| async move {
            assert!(!port_open(port).await, "the port must stay untouched");
            let face = ctl.proxy_face().expect("the main slot must have a seat");
            assert_eq!(face.borrow().addr, None, "the face must be off");
            Ok(())
        })
        .await;
        // then: the run is fine
        assert!(res.is_ok(), "{res:?}");
    }
}

mod busy_proxy_port {
    use super::*;

    #[tokio::test]
    async fn fails_the_run_with_a_bind_error_and_no_body() {
        // given: a key and a port that somebody holds
        let (busy, port) = busy_port();
        let mut run = run_config(free_port());
        run.effective_settings.net.network_key = Some(NetworkKey::generate().unwrap().encode());
        run.effective_settings.process.socks_port = port;
        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();
        // when: the host runs
        let res = run_with("proxy-busy", run, PageRole::Optional, |_, _| async move {
            flag.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await;
        // then: the error is a `BindError` on that port, the body did not start
        let err = res.expect_err("a busy port must fail the run");
        let bind = err.downcast_ref::<BindError>();
        assert_eq!(bind.map(|b| b.port), Some(port), "{err:#}");
        assert!(!ran.load(Ordering::SeqCst), "the body must not start");
        drop(busy);
    }

    #[tokio::test]
    async fn fails_a_run_without_a_key_too() {
        // given: no key (the page is the only way in) and a held port
        let (busy, port) = busy_port();
        let mut run = run_config(free_port());
        run.effective_settings.process.socks_port = port;
        // when: the host runs as the entry
        let res = run_labeled("proxy-busy-entry", run, PageRole::OnlyEntry).await;
        // then: the process still gets the `BindError`, not a page error
        let err = res.expect_err("a busy port must fail the run");
        assert!(err.downcast_ref::<BindError>().is_some(), "{err:#}");
        drop(busy);
    }

    #[tokio::test]
    async fn does_not_leave_the_page_up() {
        // given: a held proxy port and a free page port
        let (busy, port) = busy_port();
        let page = free_port();
        let mut run = run_config(page);
        run.effective_settings.process.socks_port = port;
        // when: the host runs
        let res = run_labeled("proxy-busy-page", run, PageRole::Optional).await;
        // then: it failed before the page came up
        assert!(res.is_err());
        assert!(!port_open(page).await, "the page must not be listening");
        drop(busy);
    }
}

mod process_proxy_start {
    use super::*;

    #[tokio::test]
    async fn the_main_slot_gets_the_seat_with_the_listening_port() {
        // given: a free proxy port
        let port = free_port();
        let mut run = run_config(free_port());
        run.effective_settings.process.socks_port = port;
        // when: the body looks at the controller and the port
        let res = run_with("proxy-seat", run, PageRole::Optional, move |_, ctl| async move {
            assert!(port_open(port).await, "the proxy port must be open in the body");
            let face = ctl.proxy_face().expect("the main slot must have a seat");
            assert_eq!(face.borrow().addr.map(|a| a.port()), Some(port));
            Ok(())
        })
        .await;
        // then: the body's checks held
        assert!(res.is_ok(), "{res:?}");
    }

    #[tokio::test]
    async fn the_accepting_line_is_printed_once() {
        // given: a free proxy port
        let port = free_port();
        let mut run = run_config(free_port());
        run.effective_settings.process.socks_port = port;
        let mark = log_summary(0).last_seq;
        // when: the host runs
        let res = run_labeled("proxy-once", run, PageRole::Optional).await;
        // then: one line names the port
        assert!(res.is_ok(), "{res:?}");
        let want = format!("accepting connections on 127.0.0.1:{port}");
        let lines = socks_lines("proxy-once", mark);
        let found = lines.iter().filter(|l| l.contains(&want)).count();
        assert_eq!(found, 1, "{lines:?}");
    }

    #[tokio::test]
    async fn the_port_is_free_after_the_run() {
        // given: a run that held the proxy port
        let port = free_port();
        let mut run = run_config(free_port());
        run.effective_settings.process.socks_port = port;
        let res = run_labeled("proxy-release", run, PageRole::Optional).await;
        assert!(res.is_ok(), "{res:?}");
        // when / then: the same port can be taken again at once
        let again = std::net::TcpListener::bind(("127.0.0.1", port));
        assert!(again.is_ok(), "the port was not released: {again:?}");
    }
}

mod several_networks {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_added_network_gets_the_seat_of_the_process_proxy() {
        // given: a host with the proxy listening
        let fx = setup_host("added").await;
        // when: a network is added at run time
        let id = fx.when_network_added();
        // then: its controller shows the process listener
        fx.then_slot_shows_the_listener(&id);
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_saved_network_gets_the_seat_of_the_process_proxy() {
        // given: a host with the proxy listening and a network saved on disk
        let mut fx = setup_host("saved").await;
        let id = fx.saved_network_on_disk();
        // when: the saved networks are started
        fx.when_saved_networks_started();
        // then: its controller shows the process listener
        fx.then_slot_shows_the_listener(&id);
        fx.finish().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_port_is_taken_once_for_three_networks() {
        // given: a host with the proxy listening, a saved network and an added one
        let mut fx = setup_host("once").await;
        fx.saved_network_on_disk();
        fx.when_saved_networks_started();
        fx.when_network_added();
        // when: the nodes had time to start
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // then: one accepting line for the port, and the port is still ours
        let want = format!("accepting connections on 127.0.0.1:{}", fx.port);
        let lines = socks_lines_any(fx.log_mark);
        assert_eq!(lines.iter().filter(|l| l.contains(&want)).count(), 1, "{lines:?}");
        assert_eq!(fx.slots.len(), 3, "main, saved and added");
        fx.finish().await;
    }
}

/// A host of slots with the process proxy listening and the seat handed to the host.
struct HostFx {
    dir: PathBuf,
    slots: Arc<HostSlots>,
    front: ProxyFront,
    port: u16,
    main: RunConfig,
    relaunch: Relaunch,
    log_mark: u64,
}

async fn setup_host(tag: &str) -> HostFx {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("mesh-host-proxy-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cli = CliOverrides {
        network_key: Some(NetworkKey::from_bytes([61; 32]).encode()),
        transport_mode: Some(TransportMode::NoRelay),
        checks_enabled: Some(false),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let mut main = config_file::prepare(&source, &cli, true, 0).unwrap();
    let port = free_port();
    main.effective_settings.process.socks_port = port;
    let log_mark = log_summary(0).last_seq;
    let front = ProxyFront::bind(&main.effective_settings.process).await.unwrap();
    let seat = Arc::new(ProxySeat::new(Some(front.clone()), NetSlotId::main()));
    let ctl = NodeCtl::new(&main.effective_settings);
    ctl.set_proxy_seat(NetSlotId::main(), seat.clone());
    let slots = HostSlots::with_main(ctl, ConfigState::from_run(&main));
    slots.set_proxy_seat(seat);
    let relaunch = Relaunch {
        file: main.file.clone(),
        cli,
        secret: main.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: Some(crate::transport::net::mem::process_net()),
    };
    let sources = GuardSources::default();
    let (main_run, ctx_relaunch) = (main.clone(), relaunch.clone());
    slots.set_context(SlotContext { main_run, relaunch: ctx_relaunch, sources });
    HostFx { dir, slots, front, port, main, relaunch, log_mark }
}

impl HostFx {
    fn when_network_added(&self) -> NetSlotId {
        self.slots.add_slot().expect("add a network").id
    }

    /// Writes a slot file with its own key beside the main file.
    fn saved_network_on_disk(&mut self) -> NetSlotId {
        let main_path = self.dir.join(config_file::FILE_NAME);
        let id = new_slot_id(&main_path).expect("a free slot id");
        let net = NetSettings {
            network_key: Some(NetworkKey::from_bytes([62; 32]).encode()),
            transport_mode: TransportMode::NoRelay,
            checks_enabled: false,
            ..NetSettings::default()
        };
        let path = net_slot_path(&main_path, &id).unwrap();
        let secret = NodeSecret::from_bytes(&[62; 32]);
        save_net_file(&path, None, 1_790_000_000, 1_790_000_000, &secret, &net).expect("write the slot file");
        id
    }

    fn when_saved_networks_started(&self) {
        let sources = GuardSources::default();
        self.slots.start_saved_slots(&self.main, &self.relaunch, &sources, 1_790_000_000);
    }

    /// The slot's own controller shows the process listener. The slot leaves the map and is stopped,
    /// because the map hands out only the page-facing handle.
    fn then_slot_shows_the_listener(&self, id: &NetSlotId) {
        let slot = self.slots.remove_slot(id).unwrap_or_else(|| panic!("no slot {id}"));
        let face = slot.ctl.proxy_face().expect("the slot has no proxy seat");
        let shown = face.borrow().addr;
        slot.ctl.stop_on_os_signal();
        assert_eq!(shown, self.front.addr(), "the slot must show the process listener");
        assert_eq!(shown.map(|a| a.port()), Some(self.port));
    }

    /// Stops every slot, waits for their tasks and closes the listener.
    async fn finish(&self) {
        self.slots.stop_all();
        for (id, task) in self.slots.take_tasks() {
            let done = tokio::time::timeout(std::time::Duration::from_secs(20), task).await;
            assert!(done.is_ok(), "the task of slot {id} did not stop");
        }
        self.front.close().await;
    }
}

impl Drop for HostFx {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// Runs the host with `body`; the lines of the run carry `label`, so other tests' lines don't mix in.
async fn run_with<B, F>(label: &str, run: RunConfig, role: PageRole, body: B) -> anyhow::Result<()>
where
    B: FnOnce(RunConfig, Arc<NodeCtl>) -> F + Send + 'static,
    F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let ctl = ctl_of(&run);
    let host = Host::run_with_saved_slots(run, ctl, role, None, body);
    with_net(Some(label.into()), host).await
}

/// Runs the host around a body that returns at once.
async fn run_labeled(label: &str, run: RunConfig, role: PageRole) -> anyhow::Result<()> {
    run_with(label, run, role, |_, _| async { Ok(()) }).await
}

/// The `[socks]` lines of the run labelled `label` that appeared after `seq`.
fn socks_lines(label: &str, seq: u64) -> Vec<String> {
    log_page(seq, 5000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "socks" && l.net.as_deref() == Some(label))
        .map(|l| l.text)
        .collect()
}

/// All `[socks]` lines after `seq`, whatever run printed them.
fn socks_lines_any(seq: u64) -> Vec<String> {
    log_page(seq, 5000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "socks")
        .map(|l| l.text)
        .collect()
}

/// A port somebody holds: the listener must stay alive while the test uses the port.
fn busy_port() -> (std::net::TcpListener, u16) {
    let busy = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = busy.local_addr().unwrap().port();
    (busy, port)
}
