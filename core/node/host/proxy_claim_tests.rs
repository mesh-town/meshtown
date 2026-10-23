//! Tests of choosing the network whose exit the proxy uses: switching, returning to the main network,
//! leaving the chosen network, a failed write, an unknown network and a repeated choice.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::*;
use crate::config::config_file::{
    self, list_net_slots, load_config_file, load_net_file, net_slot_path, new_slot_id,
    prepare_net_slot, save_net_file, CliOverrides, ConfigSource, LoadedConfigFile, LoadedNetFile,
    NetSettings, PathRule, RunConfig, Settings, TransportMode,
};
use crate::members::network_key::NetworkKey;
use crate::node::node_ctl::NodeCtl;
use crate::node::output::{log_page, log_summary};
use crate::proxy::ProxySeat;
use crate::transport::net::{NodeSecret, PeerId};

const NOW: u64 = 1_790_000_000;

mod switching_to_another_network {
    use super::*;

    #[tokio::test]
    async fn the_new_network_becomes_the_proxy_network() {
        // given: network A holds the proxy, network B has an exit
        let fx = setup(Holder::A);
        // when: the proxy is claimed for B
        let got = fx.when_claimed(&fx.b.clone());
        // then: the choice is B
        assert_eq!(got, Ok(()));
        assert_eq!(fx.seat.chosen(), fx.b);
    }

    #[tokio::test]
    async fn the_main_file_names_the_new_network() {
        // given: network A holds the proxy
        let fx = setup(Holder::A);
        // when: the proxy is claimed for B
        fx.when_claimed(&fx.b.clone()).unwrap();
        // then: `proxy.net` of the main file is B
        assert_eq!(fx.main_proxy_net(), fx.b);
    }

    #[tokio::test]
    async fn the_previous_network_loses_its_exits_in_file_and_state_and_the_log_says_so() {
        // given: network A holds the proxy and has the exit `x`; B has the exit `y`
        let fx = setup(Holder::A);
        assert_eq!(fx.exits_in_file(&fx.a), vec![fx.x]);
        // when: the proxy is claimed for B
        fx.when_claimed(&fx.b.clone()).unwrap();
        // then: A has no exits in its file or in its state, B keeps its exit, and the log has a line
        assert!(fx.exits_in_file(&fx.a).is_empty(), "file of A");
        assert!(fx.exits_in_state(&fx.a).is_empty(), "state of A");
        assert_eq!(fx.exits_in_file(&fx.b), vec![fx.y], "B is untouched");
        let want = format!("route exits removed in {}: 1", fx.a);
        assert!(fx.log_has(&want), "no log line {want:?}");
    }
}

mod claiming_the_network_that_already_holds_the_proxy {
    use super::*;

    #[tokio::test]
    async fn writes_nothing() {
        // given: the proxy was claimed for B
        let fx = setup(Holder::A);
        fx.when_claimed(&fx.b.clone()).unwrap();
        let before = fx.all_files();
        // when: the proxy is claimed for B again
        let got = fx.when_claimed(&fx.b.clone());
        // then: no file is written
        assert_eq!(got, Ok(()));
        assert_eq!(fx.all_files(), before, "no file may change");
    }
}

mod returning_to_the_main_network {
    use super::*;

    #[tokio::test]
    async fn clears_the_choice_and_the_exits_of_the_previous_network() {
        // given: network B holds the proxy and has the exit `y`
        let fx = setup(Holder::B);
        assert_eq!(fx.main_proxy_net(), fx.b);
        // when: the proxy is claimed for the main network
        let got = fx.when_claimed(&NetSlotId::main());
        // then: the choice is the main network, the main file has no `proxy` section, B has no exits
        assert_eq!(got, Ok(()));
        assert_eq!(fx.seat.chosen(), NetSlotId::main());
        assert_eq!(fx.main_proxy_net(), NetSlotId::main());
        assert!(
            !fx.main_text().contains("\"proxy\""),
            "the default choice is not written"
        );
        assert!(fx.exits_in_file(&fx.b).is_empty(), "file of B");
    }
}

mod leaving_the_network_that_holds_the_proxy {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn the_proxy_goes_back_to_the_main_network() {
        // given: added network B holds the proxy
        let fx = setup(Holder::B);
        // when: B is left
        fx.slots.retire(&fx.b, false).unwrap();
        finish(&fx.slots).await;
        // then: the choice is the main network, in the seat and in the main file
        assert_eq!(fx.seat.chosen(), NetSlotId::main());
        assert_eq!(fx.main_proxy_net(), NetSlotId::main());
    }

    #[tokio::test(start_paused = true)]
    async fn the_file_of_the_left_network_is_not_written() {
        // given: added network B holds the proxy and has the exit `y`
        let fx = setup(Holder::B);
        let before = fx.bytes_of(&net_slot_path(&fx.main_file(), &fx.b).unwrap());
        // when: B is left
        fx.slots.retire(&fx.b, false).unwrap();
        finish(&fx.slots).await;
        // then: the file set aside with its folder is byte for byte what it was
        let after = fx.bytes_of(&fx.set_aside_file(&fx.b));
        assert_eq!(after, before, "exits must stay in the left network's file");
    }
}

mod a_write_fails_halfway {
    use super::*;

    #[tokio::test]
    async fn main_file_not_writable_leaves_the_choice_and_the_exits_as_they_were() {
        // given: network A holds the proxy and has the exit `x`; the main file is read-only
        let mut fx = setup(Holder::A);
        fx.make_read_only(&fx.main_file());
        let before = fx.all_files();
        // when: the proxy is claimed for B
        let got = fx.when_claimed(&fx.b.clone());
        // then: an I/O error, the choice is A, the exits of A are in file and state, nothing is written
        assert!(matches!(got, Err(HostError::Io(_))), "{got:?}");
        assert_eq!(fx.seat.chosen(), fx.a);
        assert_eq!(fx.exits_in_file(&fx.a), vec![fx.x], "file of A");
        assert_eq!(fx.exits_in_state(&fx.a), vec![fx.x], "state of A");
        assert_eq!(fx.all_files(), before, "no file may change");
        assert!(!fx.log_has("route exits removed"), "A was not touched");
    }

    #[tokio::test]
    async fn previous_network_not_writable_keeps_the_new_choice_and_logs_it() {
        // given: network A holds the proxy and has the exit `x`; the file of A is read-only
        let mut fx = setup(Holder::A);
        fx.make_read_only(&net_slot_path(&fx.main_file(), &fx.a).unwrap());
        // when: the proxy is claimed for B
        let got = fx.when_claimed(&fx.b.clone());
        // then: the choice moved to B, the exit of A stays in its file, and the log has a line about A
        assert_eq!(got, Ok(()));
        assert_eq!(fx.seat.chosen(), fx.b);
        assert_eq!(fx.main_proxy_net(), fx.b);
        assert_eq!(fx.exits_in_file(&fx.a), vec![fx.x]);
        let want = format!("route exits not removed in {}", fx.a);
        assert!(fx.log_has(&want), "no log line {want:?}");
    }
}

mod an_unknown_network {
    use super::*;

    #[tokio::test]
    async fn is_an_error_and_the_choice_stays() {
        // given: network A holds the proxy
        let fx = setup(Holder::A);
        let before = fx.all_files();
        let unknown = NetSlotId::parse("aaaaaa").unwrap();
        // when: the proxy is claimed for a network that is not loaded
        let got = fx.when_claimed(&unknown);
        // then: an error, the choice is A, no file is written
        // QUESTION: the error kind is assumed to be `UnknownNet` (the plan says only "an error").
        assert_eq!(got, Err(HostError::UnknownNet));
        assert_eq!(fx.seat.chosen(), fx.a);
        assert_eq!(fx.all_files(), before, "no file may change");
    }
}

/// Which network holds the proxy at the start.
#[derive(Clone, Copy)]
enum Holder {
    A,
    B,
}

/// A main slot without a node and two added slots `a` (exit `x`) and `b` (exit `y`), each with a settings file.
struct Fixture {
    dir: PathBuf,
    slots: Arc<HostSlots>,
    seat: Arc<ProxySeat>,
    main_ctl: Arc<NodeCtl>,
    main_run: RunConfig,
    a: NetSlotId,
    b: NetSlotId,
    x: PeerId,
    y: PeerId,
    log_mark: u64,
    read_only: Vec<PathBuf>,
}

fn setup(holder: Holder) -> Fixture {
    let dir = temp_dir();
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let overrides = CliOverrides::default();
    let main_run = config_file::prepare(&source, &overrides, true, 0).unwrap();
    let main_ctl = NodeCtl::new(&main_run.effective_settings);
    let state = crate::web_gui::ConfigState::from_run(&main_run);
    let slots = HostSlots::with_main(main_ctl.clone(), state);
    let seat = Arc::new(ProxySeat::new(None, NetSlotId::main()));
    let mut fx = Fixture {
        dir,
        slots,
        seat,
        main_ctl,
        main_run,
        a: NetSlotId::main(),
        b: NetSlotId::main(),
        x: peer(1),
        y: peer(2),
        log_mark: log_summary(0).last_seq,
        read_only: Vec::new(),
    };
    fx.a = fx.add_slot(11, vec![fx.x]);
    fx.b = fx.add_slot(12, vec![fx.y]);
    let holder_id = match holder {
        Holder::A => fx.a.clone(),
        Holder::B => fx.b.clone(),
    };
    fx.hold_proxy(holder_id);
    fx
}

impl Fixture {
    fn main_file(&self) -> PathBuf {
        self.dir.join(config_file::FILE_NAME)
    }

    fn when_claimed(&self, id: &NetSlotId) -> Result<(), HostError> {
        self.slots.claim(id, true)
    }

    /// Adds a running slot whose file holds the given exits.
    fn add_slot(&mut self, seed: u8, exits: Vec<PeerId>) -> NetSlotId {
        let main = self.main_file();
        let id = new_slot_id(&main).unwrap();
        let net = NetSettings {
            network_key: Some(NetworkKey::from_bytes([seed; 32]).encode()),
            transport_mode: TransportMode::NoRelay,
            checks_enabled: false,
            route_exits: exits,
            ..NetSettings::default()
        };
        let path = net_slot_path(&main, &id).unwrap();
        let secret = NodeSecret::from_bytes(&[seed; 32]);
        save_net_file(&path, None, NOW, NOW, &secret, &net).unwrap();
        let on_disk = list_net_slots(&main).into_iter().find(|s| s.id == id);
        let run = prepare_net_slot(&self.main_run, on_disk.unwrap(), NOW).unwrap();
        let ctl = NodeCtl::new(&run.effective_settings);
        let handle = crate::web_gui::SlotHandle {
            id: id.clone(),
            node: ctl.clone(),
            config: Arc::new(Mutex::new(crate::web_gui::ConfigState::from_run(&run))),
        };
        let waiting = ctl.clone();
        let cycle = async move {
            waiting.next_command().await;
            Ok(())
        };
        self.slots.push_slot(handle, ctl, cycle).unwrap();
        id
    }

    /// Makes `id` the proxy network in the main file and in a fresh seat.
    fn hold_proxy(&mut self, id: NetSlotId) {
        let main = self.slots.main_slot();
        let mut state = main.config.lock().unwrap();
        let edit = |s: &mut Settings| s.process.proxy_net = id.clone();
        state.rewrite_settings(self.main_ctl.as_ref(), edit).unwrap();
        drop(state);
        self.seat = Arc::new(ProxySeat::new(None, id));
        self.slots.set_proxy_seat(self.seat.clone());
    }

    fn main_text(&self) -> String {
        std::fs::read_to_string(self.main_file()).unwrap()
    }

    fn main_proxy_net(&self) -> NetSlotId {
        match load_config_file(&self.main_file()) {
            LoadedConfigFile::Parsed(p) => p.settings.process.proxy_net,
            _ => panic!("the main file can't be read"),
        }
    }

    fn exits_in_file(&self, id: &NetSlotId) -> Vec<PeerId> {
        match load_net_file(&net_slot_path(&self.main_file(), id).unwrap()) {
            LoadedNetFile::Parsed(p) => p.net.route_exits,
            _ => panic!("the file of {id} can't be read"),
        }
    }

    fn exits_in_state(&self, id: &NetSlotId) -> Vec<PeerId> {
        let handle = self.slots.slot(id).unwrap();
        let state = handle.config.lock().unwrap();
        state.file_settings.net.route_exits.clone()
    }

    fn bytes_of(&self, path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }

    /// The bytes of the main file and of both slot files.
    fn all_files(&self) -> Vec<Vec<u8>> {
        let main = self.main_file();
        let slot = |id: &NetSlotId| self.bytes_of(&net_slot_path(&main, id).unwrap());
        vec![self.bytes_of(&main), slot(&self.a), slot(&self.b)]
    }

    /// The settings file of a left network, in the folder that was set aside.
    fn set_aside_file(&self, id: &NetSlotId) -> PathBuf {
        let prefix = format!("left-{id}-");
        let found = std::fs::read_dir(self.dir.join("nets"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix)));
        found.expect("the folder of the left network").join(config_file::FILE_NAME)
    }

    fn make_read_only(&mut self, path: &Path) {
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(path, perms).unwrap();
        self.read_only.push(path.to_path_buf());
    }

    fn log_has(&self, text: &str) -> bool {
        let lines = log_page(self.log_mark, 5000).lines;
        lines.iter().any(|l| l.text.contains(text))
    }
}

impl Drop for Fixture {
    #[allow(clippy::permissions_set_readonly_false)]
    fn drop(&mut self) {
        for path in &self.read_only {
            if let Ok(meta) = std::fs::metadata(path) {
                let mut perms = meta.permissions();
                perms.set_readonly(false);
                std::fs::set_permissions(path, perms).ok();
            }
        }
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-proxy-claim-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

async fn finish(slots: &HostSlots) {
    for task in slots.take_retiring() {
        task.await.unwrap();
    }
}
