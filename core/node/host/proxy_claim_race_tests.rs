//! Tests of choosing the proxy network under contention: which locks `claim` holds while it works, two
//! claims at once, leaving a network while it is being claimed, the start-up lines about a choice that
//! cannot work, and the path the proxy takes before and after the choice.

use std::path::PathBuf;
use std::sync::{Arc, Barrier, Mutex};
use std::time::{Duration, Instant};

use crate::config::config_file::{
    self, list_net_slots, load_config_file, load_net_file, net_slot_path, new_slot_id,
    prepare_net_slot, save_net_file, CliOverrides, ConfigSource, LoadedConfigFile, LoadedNetFile,
    NetSettings, NetSlotId, PathRule, ProcessSettings, RunConfig, Settings, TransportMode,
};
use crate::exit::PeerConnPool;
use crate::members::network_key::NetworkKey;
use crate::node::host::slots::HostSlots;
use crate::node::node_ctl::NodeCtl;
use crate::node::output::{log_page, log_summary};
use crate::node::RunningNode;
use crate::proxy::route::RouteCtx;
use crate::proxy::{ProxyConfig, ProxyFront, ProxySeat};
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::{NodeSecret, PeerId};
use crate::web_gui::{ConfigState, HostControl, HostError, SlotHandle};

const NOW: u64 = 1_790_000_000;
const ROUNDS: usize = 20;
const PATIENCE: Duration = Duration::from_secs(5);

mod claim_while_the_previous_network_is_busy {
    use super::*;

    /// given the previous network's config state is held by someone else, when another network is claimed,
    /// then the choice is made and the main file is written without waiting for it, and the main
    /// network's own lock is free; the exits of the previous network go once its lock is released.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn main_is_written_and_chosen_before_the_previous_lock_is_needed() {
        // given
        let mut fx = setup();
        let previous = fx.add_slot(11, &[1]);
        let next = fx.add_slot(12, &[2]);
        fx.hold_proxy(&previous);
        let held = fx.slots.slot(&previous).unwrap().config;
        let guard = held.lock().unwrap();

        // when
        let claim = fx.claim_in_thread(&next);

        // then
        assert!(
            wait_until(|| fx.seat.chosen() == next),
            "the choice must not wait for the previous network's lock"
        );
        assert_eq!(fx.main_proxy_net(), next, "the main file is already written");
        assert!(
            fx.slots.main_slot().config.try_lock().is_ok(),
            "the main network's lock must not be held while the previous one is awaited"
        );
        assert_eq!(fx.exits_in_file(&previous), vec![peer(1)], "not cleared yet");
        drop(guard);
        assert_eq!(claim.join().unwrap(), Ok(()));
        assert!(fx.exits_in_file(&previous).is_empty(), "cleared after the lock");
    }
}

mod two_claims_at_once {
    use super::*;

    /// given two networks with exits, when each is claimed from its own thread, then one of them holds the
    /// choice and only it keeps its exits.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn exactly_one_network_keeps_its_exits() {
        for round in 0..ROUNDS {
            // given
            let mut fx = setup();
            let first = fx.add_slot(21, &[1]);
            let second = fx.add_slot(22, &[2]);

            // when
            let answers = in_parallel(vec![fx.claimer(&first), fx.claimer(&second)]);

            // then
            assert_eq!(answers, vec![Ok(()), Ok(())], "round {round}");
            let chosen = fx.seat.chosen();
            assert!(chosen == first || chosen == second, "round {round}: {chosen}");
            assert_eq!(fx.main_proxy_net(), chosen, "round {round}: the main file");
            let holding: Vec<NetSlotId> = [&first, &second]
                .into_iter()
                .filter(|id| !fx.exits_in_file(id).is_empty())
                .cloned()
                .collect();
            assert_eq!(holding, vec![chosen], "round {round}: who still has exits");
        }
    }
}

mod leaving_while_being_claimed {
    use super::*;

    /// given a network with exits, when it is left and claimed at the same time, then the removed network
    /// does not stay chosen, in the seat or in the main file.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_removed_network_is_not_left_chosen() {
        for round in 0..ROUNDS {
            // given
            let mut fx = setup();
            let doomed = fx.add_slot(31, &[1]);

            // when
            in_parallel(vec![fx.claimer(&doomed), fx.leaver(&doomed)]);
            finish(&fx.slots).await;

            // then
            assert!(fx.slots.slot(&doomed).is_none(), "round {round}: it left");
            assert_ne!(fx.seat.chosen(), doomed, "round {round}: the seat");
            assert_eq!(fx.main_proxy_net(), NetSlotId::main(), "round {round}: the main file");
        }
    }
}

mod start_lines {
    use super::*;

    /// given `proxy.net` names a network that is not loaded, then the start says the proxy refuses
    /// connections and the front has no target.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_unloaded_network_is_named_and_the_front_has_no_target() {
        // given
        let front = new_front().await;
        let mut fx = setup_with(Some(front.clone()));
        let ghost = new_slot_id(&fx.main_file()).unwrap();
        fx.hold_proxy(&ghost);

        // when
        fx.slots.report_proxy_start();

        // then
        let want = format!("network {ghost} is not loaded; the proxy refuses connections");
        assert_eq!(fx.proxy_lines_about(&[&ghost]), vec![want]);
        assert!(front.current_route().is_none(), "a direct path would leak");
    }

    /// given two networks besides the chosen one keep exits, then each gets one line and no file changes.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn every_other_network_with_exits_gets_a_line_and_files_stay_as_they_were() {
        // given
        let mut fx = setup();
        let first = fx.add_slot(41, &[1]);
        let second = fx.add_slot(42, &[2]);
        let quiet = fx.add_slot(43, &[]);
        let files = fx.all_files();

        // when
        fx.slots.report_proxy_start();

        // then
        let line = |id: &NetSlotId| {
            format!("network {id} has route exits, but the proxy serves main; they are not used")
        };
        let lines = fx.proxy_lines_about(&[&first, &second, &quiet]);
        assert_eq!(lines, vec![line(&first), line(&second)]);
        assert_eq!(fx.all_files(), files, "the files are not touched");
    }

    /// given only the chosen network has exits and the chosen one is loaded, then the start is silent.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nothing_to_report_is_silent() {
        // given
        let mut fx = setup();
        let chosen = fx.add_slot(51, &[1]);
        let other = fx.add_slot(52, &[]);
        fx.hold_proxy(&chosen);

        // when
        fx.slots.report_proxy_start();

        // then
        let lines = fx.proxy_lines_about(&[&chosen, &other]);
        assert!(lines.is_empty(), "unexpected lines: {lines:?}");
    }
}

mod path_of_the_proxy {
    use super::*;

    /// given the main network has no exits and another one has an exit, when the other is claimed, then the
    /// front's target changes from the direct path of the main network to the exit of the other.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn follows_the_claimed_network_from_direct_to_its_exit() {
        // given
        let front = new_front().await;
        let mut fx = setup_with(Some(front.clone()));
        let other = fx.add_slot(61, &[9]);
        let node = start_loopback_node().await;
        fx.seat.node_up(&NetSlotId::main(), proxy_config(&node, &[]));
        fx.seat.node_up(&other, proxy_config(&node, &[9]));
        assert_eq!(chosen_exits_of(&front), Some(vec![]), "the main network, direct");

        // when
        fx.slots.claim(&other, true).unwrap();

        // then
        assert_eq!(chosen_exits_of(&front), Some(vec![peer(9)]), "the exit of the claimed network");
        front.close().await;
    }
}

struct Fixture {
    dir: PathBuf,
    slots: Arc<HostSlots>,
    seat: Arc<ProxySeat>,
    main_ctl: Arc<NodeCtl>,
    main_run: RunConfig,
    log_mark: u64,
}

fn setup() -> Fixture {
    setup_with(None)
}

/// A host with the main slot (a settings file, no node) and a seat over `front` that has the main network chosen.
fn setup_with(front: Option<ProxyFront>) -> Fixture {
    let dir = temp_dir();
    let source = ConfigSource::File {
        path: dir.join(config_file::FILE_NAME),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let main_run = config_file::prepare(&source, &CliOverrides::default(), true, 0).unwrap();
    let main_ctl = NodeCtl::new(&main_run.effective_settings);
    let slots = HostSlots::with_main(main_ctl.clone(), ConfigState::from_run(&main_run));
    let seat = Arc::new(ProxySeat::new(front, NetSlotId::main()));
    slots.set_proxy_seat(seat.clone());
    Fixture {
        dir,
        slots,
        seat,
        main_ctl,
        main_run,
        log_mark: log_summary(0).last_seq,
    }
}

impl Fixture {
    fn main_file(&self) -> PathBuf {
        self.dir.join(config_file::FILE_NAME)
    }

    /// Adds a running slot whose file holds the exits `exits` (marked by their seeds).
    fn add_slot(&mut self, seed: u8, exits: &[u8]) -> NetSlotId {
        let main = self.main_file();
        let id = new_slot_id(&main).unwrap();
        let net = NetSettings {
            network_key: Some(NetworkKey::from_bytes([seed; 32]).encode()),
            transport_mode: TransportMode::NoRelay,
            checks_enabled: false,
            route_exits: exits.iter().map(|s| peer(*s)).collect(),
            ..NetSettings::default()
        };
        let path = net_slot_path(&main, &id).unwrap();
        let secret = NodeSecret::from_bytes(&[seed; 32]);
        save_net_file(&path, None, NOW, NOW, &secret, &net).unwrap();
        let on_disk = list_net_slots(&main).into_iter().find(|s| s.id == id);
        let run = prepare_net_slot(&self.main_run, on_disk.unwrap(), NOW).unwrap();
        let ctl = NodeCtl::new(&run.effective_settings);
        let handle = SlotHandle {
            id: id.clone(),
            node: ctl.clone(),
            config: Arc::new(Mutex::new(ConfigState::from_run(&run))),
        };
        let waiting = ctl.clone();
        let cycle = async move {
            waiting.next_command().await;
            Ok(())
        };
        self.slots.push_slot(handle, ctl, cycle).unwrap();
        id
    }

    /// Makes `id` the proxy network as a finished earlier choice would have left it: in the main file and the seat.
    fn hold_proxy(&mut self, id: &NetSlotId) {
        let main = self.slots.main_slot();
        let edit = |s: &mut Settings| s.process.proxy_net = id.clone();
        let mut state = main.config.lock().unwrap();
        state.rewrite_settings(self.main_ctl.as_ref(), edit).unwrap();
        drop(state);
        self.seat.choose(id.clone());
    }

    fn claim_in_thread(&self, id: &NetSlotId) -> std::thread::JoinHandle<Result<(), HostError>> {
        let (job, rt) = (self.claimer(id), tokio::runtime::Handle::current());
        std::thread::spawn(move || {
            let _inside = rt.enter();
            job()
        })
    }

    fn claimer(&self, id: &NetSlotId) -> Job {
        let (slots, id) = (self.slots.clone(), id.clone());
        Box::new(move || slots.claim(&id, true))
    }

    fn leaver(&self, id: &NetSlotId) -> Job {
        let (slots, id) = (self.slots.clone(), id.clone());
        Box::new(move || slots.retire_slot(&id, false))
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

    /// The bytes of the main file and of every slot file.
    fn all_files(&self) -> Vec<Vec<u8>> {
        let main = self.main_file();
        let mut files = vec![std::fs::read(&main).unwrap()];
        for slot in list_net_slots(&main) {
            files.push(std::fs::read(net_slot_path(&main, &slot.id).unwrap()).unwrap());
        }
        files
    }

    /// The proxy lines of the journal since the fixture was made that name one of `ids`, in order.
    fn proxy_lines_about(&self, ids: &[&NetSlotId]) -> Vec<String> {
        let names: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
        let lines = log_page(self.log_mark, 5000).lines;
        let ours = lines.into_iter().filter(|l| l.tag == "proxy");
        let named = ours.filter(|l| names.iter().any(|n| l.text.contains(n.as_str())));
        named.map(|l| l.text).collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

type Job = Box<dyn FnOnce() -> Result<(), HostError> + Send>;

/// Runs the jobs from their own threads, released together; returns their answers in order.
fn in_parallel(jobs: Vec<Job>) -> Vec<Result<(), HostError>> {
    let gate = Arc::new(Barrier::new(jobs.len()));
    let rt = tokio::runtime::Handle::current();
    let threads: Vec<_> = jobs
        .into_iter()
        .map(|job| {
            let (gate, rt) = (gate.clone(), rt.clone());
            std::thread::spawn(move || {
                let _inside = rt.enter();
                gate.wait();
                job()
            })
        })
        .collect();
    threads.into_iter().map(|t| t.join().unwrap()).collect()
}

/// Polls `ready` until it holds or the patience runs out.
fn wait_until(ready: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < PATIENCE {
        if ready() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    ready()
}

async fn finish(slots: &HostSlots) {
    for task in slots.take_retiring() {
        task.await.unwrap();
    }
}

fn temp_dir() -> PathBuf {
    static COUNTER: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mesh-proxy-race-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn peer(seed: u8) -> PeerId {
    NodeSecret::from_bytes(&[seed; 32]).peer_id()
}

async fn new_front() -> ProxyFront {
    let settings = ProcessSettings {
        socks_port: 0,
        ..ProcessSettings::default()
    };
    ProxyFront::bind(&settings).await.expect("a free loopback port")
}

async fn start_loopback_node() -> RunningNode {
    let key = NetworkKey::generate().unwrap();
    crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap()
}

/// A proxy configuration whose chosen exits are the peers marked by `exits`.
fn proxy_config(node: &RunningNode, exits: &[u8]) -> ProxyConfig {
    let resolver = Resolver::new(
        std::net::SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a stub address");
    ProxyConfig {
        route: RouteCtx::new(
            node.members.clone(),
            node.domains.clone(),
            node.id,
            exits.iter().map(|s| peer(*s)).collect(),
        ),
        stack: node.stack.clone(),
        pool: PeerConnPool::default(),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(Default::default()),
        udp_slots: crate::proxy::udp_slots(),
    }
}

/// The exits the front's current target routes through; `None` — no target.
fn chosen_exits_of(front: &ProxyFront) -> Option<Vec<PeerId>> {
    front.current_route().map(|route| route.chosen_exits())
}
