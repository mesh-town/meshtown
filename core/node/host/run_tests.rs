//! Tests of the process host: the order "log capacity → page → body → page shutdown",
//! the page roles and the error texts. The network is 127.0.0.1 only.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::{Host, PageRole};
use crate::config::config_file::{self, CliOverrides, ConfigSource, ListenAccess, RunConfig};
use crate::node::node_ctl::NodeCtl;
use crate::node::output::{log_page, log_summary, say, set_log_capacity};

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
