//! Integration tests of the host's start of the networks saved under `nets/`: the launch path hands the
//! saved slots to the host only when the caller passes a `Relaunch`, and reads `nets/` beside the main file.
//!
//! What it does: runs `Host::run_with_saved_slots` / `run_with_web_page` with a main file and a damaged
//! slot file beside it, and reads the log. The page is off, no node is started.
//! What it doesn't do: it doesn't start slot nodes (the host unit tests in `core/node/host/` do, on `mem`).
//! Links: `meshtown_core::node::host::Host`.

use std::path::{Path, PathBuf};

use meshtown_core::config::config_file::{
    self, net_slot_path, CliOverrides, ConfigSource, NetSlotId, PathRule, RunConfig,
};
use meshtown_core::node::host::{Host, PageRole};
use meshtown_core::node::node_ctl::{NodeCtl, Relaunch};
use meshtown_core::node::output::{log_page, log_summary};
use meshtown_core::transport::net::NodeSecret;

const BROKEN_TEXT: &str = "this is not a slot file";

mod damaged_slot_file {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_reported_when_the_launch_hands_over_a_relaunch() {
        // given: a main file with a damaged slot file beside it
        let fx = setup("reported");
        let mark = log_summary(0).last_seq;
        // when: the host runs with a `Relaunch`
        fx.run(true).await;
        // then: the log says the slot was not started
        let want = format!("net slot {} not started", fx.slot);
        assert!(lines_after(mark).contains(&want), "no line: {want}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_left_untouched() {
        // given: a main file with a damaged slot file beside it
        let fx = setup("untouched");
        // when: the host runs with a `Relaunch`
        fx.run(true).await;
        // then: the slot file is the same, nothing was set aside
        assert_eq!(std::fs::read_to_string(&fx.slot_file).unwrap(), BROKEN_TEXT);
        assert_eq!(std::fs::read_dir(fx.dir.join("nets")).unwrap().count(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_not_read_without_a_relaunch() {
        // given: the same folder
        let fx = setup("no-relaunch");
        let mark = log_summary(0).last_seq;
        // when: the host runs without a `Relaunch`
        fx.run(false).await;
        // then: `nets/` was not looked at
        let want = format!("net slot {}", fx.slot);
        assert!(!lines_after(mark).contains(&want), "the slot was read");
    }
}

struct Fixture {
    dir: PathBuf,
    run: RunConfig,
    slot: NetSlotId,
    slot_file: PathBuf,
}

impl Fixture {
    /// Runs the host around a main body that returns at once.
    async fn run(&self, with_relaunch: bool) {
        let ctl = NodeCtl::new(&self.run.effective_settings);
        let relaunch = with_relaunch.then(|| Relaunch {
            file: self.run.file.clone(),
            cli: CliOverrides::default(),
            secret: self.run.secret.clone(),
            initial_domains: Vec::new(),
            blocked_domains: Vec::new(),
            test_net: Some(meshtown_core::transport::net::mem::process_net()),
        });
        let res = Host::run_with_saved_slots(
            self.run.clone(),
            ctl,
            PageRole::Optional,
            relaunch,
            |_, _| async { Ok(()) },
        )
        .await;
        assert!(res.is_ok(), "{res:?}");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// A main file in a fresh folder (page off) and a damaged slot file `nets/<id>/config.json` beside it.
fn setup(tag: &str) -> Fixture {
    let dir = std::env::temp_dir().join(format!("mesh-net-slots-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary test directory");
    let overrides = CliOverrides {
        network_key: Some(meshtown_core::members::network_key::managed_key_with_label(
            None,
            &[4u8; 32],
            &NodeSecret::from_bytes(&[31; 32]).peer_id(),
        )),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let mut run = config_file::prepare(&source, &overrides, true, 0).expect("prepare");
    run.effective_settings.process.web_enabled = false;
    run.effective_settings.process.socks_port = 0;
    let slot = NetSlotId::parse(&unique_slot_text()).unwrap();
    let slot_file = net_slot_path(run.file.as_ref().unwrap().path(), &slot).unwrap();
    write_text(&slot_file, BROKEN_TEXT);
    Fixture {
        dir,
        run,
        slot,
        slot_file,
    }
}

/// A slot id no other test of this process uses, so that log lines of parallel tests never name it.
fn unique_slot_text() -> String {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let letter = |shift: u32| char::from(b'a' + ((n >> shift) % 26) as u8);
    format!("ab2c{}{}", letter(0), letter(5))
}

fn write_text(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// All log lines that appeared after `seq`, as one text.
fn lines_after(seq: u64) -> String {
    let texts: Vec<String> = log_page(seq, 5000)
        .lines
        .into_iter()
        .map(|l| l.text)
        .collect();
    texts.join("\n")
}
