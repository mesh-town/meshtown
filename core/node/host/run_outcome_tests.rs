//! The outcome of a whole process run and the label of the main slot's log lines: a failing saved slot
//! is a log line and does not end the process, a failing main slot is the error of the run.
//! The network is `mem` only; the page and the process proxy listen on 127.0.0.1.

use std::path::PathBuf;
use std::time::Duration;

use super::*;

/// How long the main cycle waits for the failing slot to report before it gives up.
const SLOT_REPORT_WAIT: Duration = Duration::from_secs(30);

mod a_saved_slot_that_fails {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn does_not_fail_the_process() {
        // given: a main slot that ends well once the saved slot has failed
        let mut fx = setup_run("slot-fails");
        let id = fx.saved_network_that_cannot_start();
        // when: the process runs
        let res = fx.when_run_until_slot_reports(&id).await;
        // then: the outcome of the process is the main slot's
        assert!(
            res.is_ok(),
            "the slot's failure must not fail the process: {res:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn is_reported_in_the_log_under_its_own_label() {
        // given: a main slot that ends well once the saved slot has failed
        let mut fx = setup_run("slot-line");
        let id = fx.saved_network_that_cannot_start();
        // when: the process runs
        let res = fx.when_run_until_slot_reports(&id).await;
        // then: one line says the slot stopped with an error, and it carries the slot's label
        assert!(res.is_ok(), "{res:?}");
        let lines = fx.slot_error_lines(&id);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].net.as_deref(), Some(id.to_string().as_str()));
    }
}

mod the_main_slot_that_fails {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn gives_its_error_to_the_caller() {
        // given: a main slot that ends with an error, and a saved slot that fails too
        let mut fx = setup_run("main-fails");
        fx.saved_network_that_cannot_start();
        // when: the process runs
        let res = fx
            .when_run_with(|_, _| async { Err(anyhow::anyhow!("main broke")) })
            .await;
        // then: the run returns the main slot's error
        assert_eq!(res.unwrap_err().to_string(), "main broke");
    }
}

mod main_slot_log_lines {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn carry_the_main_label() {
        // given: a main slot that writes a line of its own
        let fx = setup_run("main-label");
        let mark = log_summary(0).last_seq;
        // when: the process runs
        let res = fx
            .when_run_with(|_, _| async {
                say("test", "main slot line from the label test");
                Ok(())
            })
            .await;
        // then: the line is labelled `main`
        assert!(res.is_ok(), "{res:?}");
        let lines: Vec<_> = lines_with(mark, "main slot line from the label test");
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0].net.as_deref(), Some("main"));
    }
}

/// A process run with a settings file and room for saved networks beside it.
struct RunFx {
    dir: PathBuf,
    main: RunConfig,
    relaunch: Relaunch,
    log_mark: u64,
}

fn setup_run(tag: &str) -> RunFx {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!(
        "mesh-host-outcome-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let cli = CliOverrides {
        web_port: Some(free_port()),
        network_key: Some(crate::node::host::test_keys::managed_text(71)),
        checks_enabled: Some(false),
        ..CliOverrides::default()
    };
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let mut main = config_file::prepare(&source, &cli, true, 0).unwrap();
    main.effective_settings.process.web_access = ListenAccess::Localhost;
    main.effective_settings.process.log_ring_lines = TEST_RING;
    main.effective_settings.process.socks_port = 0;
    let relaunch = Relaunch {
        file: main.file.clone(),
        cli,
        secret: main.secret.clone(),
        initial_domains: Vec::new(),
        blocked_domains: Vec::new(),
        test_net: Some(crate::transport::net::mem::process_net()),
    };
    let log_mark = log_summary(0).last_seq;
    RunFx {
        dir,
        main,
        relaunch,
        log_mark,
    }
}

impl RunFx {
    /// Writes a slot file with its own key, and a plain file where the node must create the
    /// directory of its replicas: the node of this slot cannot start.
    fn saved_network_that_cannot_start(&mut self) -> NetSlotId {
        let main_path = self.dir.join(config_file::FILE_NAME);
        let id = new_slot_id(&main_path).expect("a free slot id");
        let net = NetSettings {
            network_key: Some(crate::node::host::test_keys::managed_text(72)),
            checks_enabled: false,
            ..NetSettings::default()
        };
        let path = net_slot_path(&main_path, &id).unwrap();
        let secret = NodeSecret::from_bytes(&[72; 32]);
        save_net_file(&path, None, 1_790_000_000, 1_790_000_000, &secret, &net)
            .expect("write the slot file");
        std::fs::write(path.parent().unwrap().join("replica"), b"not a directory").unwrap();
        id
    }

    /// Runs the host around `body` with the saved networks started beside the main slot.
    async fn when_run_with<B, F>(&self, body: B) -> anyhow::Result<()>
    where
        B: FnOnce(RunConfig, Arc<NodeCtl>) -> F + Send + 'static,
        F: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
    {
        let ctl = NodeCtl::new(&self.main.effective_settings);
        Host::run_with_saved_slots(
            self.main.clone(),
            ctl,
            PageRole::Optional,
            Some(self.relaunch.clone()),
            body,
        )
        .await
    }

    /// The main cycle ends well as soon as the log says the slot `id` stopped with an error.
    async fn when_run_until_slot_reports(&self, id: &NetSlotId) -> anyhow::Result<()> {
        let (mark, id) = (self.log_mark, id.clone());
        self.when_run_with(move |_, _| async move {
            let deadline = std::time::Instant::now() + SLOT_REPORT_WAIT;
            while slot_error_lines(mark, &id).is_empty() {
                anyhow::ensure!(
                    std::time::Instant::now() < deadline,
                    "slot {id} never reported a failure"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Ok(())
        })
        .await
    }

    fn slot_error_lines(&self, id: &NetSlotId) -> Vec<crate::web_gui::LogLine> {
        slot_error_lines(self.log_mark, id)
    }
}

impl Drop for RunFx {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// The lines after `seq` that say the slot `id` stopped with an error.
fn slot_error_lines(seq: u64, id: &NetSlotId) -> Vec<crate::web_gui::LogLine> {
    lines_with(seq, &format!("net slot {id} stopped with an error"))
}

/// The lines after `seq` whose text contains `needle`.
fn lines_with(seq: u64, needle: &str) -> Vec<crate::web_gui::LogLine> {
    let page = log_page(seq, 5000);
    page.lines
        .into_iter()
        .filter(|l| l.text.contains(needle))
        .collect()
}
