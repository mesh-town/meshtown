//! Tests of the proxy console (`console.rs`): line parsing, and the route and address commands over the process listener.

use super::*;

use crate::config::config_file::ProcessSettings;
use crate::exit::PeerConnPool;
use crate::node::output::{log_page, log_summary};
use crate::proxy::front::ProxyFront;
use crate::proxy::ProxyConfig;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::{NodeSecret, PeerId};

#[test]
fn parse_proxy_line_reads_four_commands() {
    assert_eq!(
        parse_proxy_line("socks").unwrap().unwrap(),
        ProxyCommand::Socks
    );
    assert_eq!(
        parse_proxy_line("route example.com").unwrap().unwrap(),
        ProxyCommand::Route("example.com".to_string())
    );
    assert_eq!(
        parse_proxy_line("get example.com").unwrap().unwrap(),
        ProxyCommand::Get("example.com".to_string())
    );
    assert_eq!(
        parse_proxy_line("exit on").unwrap().unwrap(),
        ProxyCommand::Exit(true)
    );
    assert_eq!(
        parse_proxy_line("exit off").unwrap().unwrap(),
        ProxyCommand::Exit(false)
    );
}

#[test]
fn parse_proxy_line_reports_wrong_arguments() {
    assert!(parse_proxy_line("route").unwrap().is_err());
    assert!(parse_proxy_line("get a b").unwrap().is_err());
    assert!(parse_proxy_line("exit maybe").unwrap().is_err());
    assert!(parse_proxy_line("socks now").unwrap().is_err());
}

#[test]
fn parse_proxy_line_ignores_alien_lines() {
    assert!(parse_proxy_line("domains list").is_none());
    assert!(parse_proxy_line("checks").is_none());
    assert!(parse_proxy_line("").is_none());
}

mod route {
    use super::*;

    mod front_has_a_target {
        use super::*;

        #[tokio::test]
        async fn prints_the_path_of_the_front_target() {
            // given
            let fx = setup(Front::Targeting(Path::Direct)).await;

            // when
            let said = fx.when_route("direct-target.console-test.example").await;

            // then
            assert_eq!(said, ["direct-target.console-test.example → direct"]);
        }

        #[tokio::test]
        async fn prints_a_refusal_of_an_offline_exit_target() {
            let fx = setup(Front::Targeting(Path::OfflineExit)).await;

            let said = fx.when_route("offline-target.console-test.example").await;

            assert_eq!(
                said,
                ["offline-target.console-test.example → refused: chosen exits are offline"]
            );
        }

        /// The target changes under the console: the next command sees the new path.
        #[tokio::test]
        async fn follows_the_target_when_it_is_replaced() {
            let fx = setup(Front::Targeting(Path::Direct)).await;
            fx.front.set_target(Some(fx.config(Path::OfflineExit)));

            let said = fx.when_route("replaced-target.console-test.example").await;

            assert_eq!(
                said,
                ["replaced-target.console-test.example → refused: chosen exits are offline"]
            );
        }
    }

    mod front_has_no_target {
        use super::*;

        #[tokio::test]
        async fn says_there_is_no_network_for_the_proxy() {
            let fx = setup(Front::Idle).await;

            let said = fx.when_route("no-target.console-test.example").await;

            assert_eq!(
                said,
                ["no-target.console-test.example → refused: no network for the proxy"]
            );
        }

        #[tokio::test]
        async fn says_it_again_after_the_target_is_taken_away() {
            let fx = setup(Front::Targeting(Path::Direct)).await;
            fx.front.set_target(None);

            let said = fx.when_route("taken-away.console-test.example").await;

            assert_eq!(
                said,
                ["taken-away.console-test.example → refused: no network for the proxy"]
            );
        }
    }
}

mod socks {
    use super::*;

    /// The address shown is the one the front listens on.
    #[tokio::test]
    async fn prints_the_address_of_the_front() {
        let fx = setup(Front::Idle).await;
        let listening = fx.front.addr().expect("the front listens");

        let said = fx.when_socks().await;

        assert_eq!(
            said,
            [format!("listening: {listening}; exit for others: off")]
        );
    }

    /// The front is off in the settings (no address): the console says `off`.
    #[tokio::test]
    async fn says_off_when_the_front_listens_nowhere() {
        let fx = setup(Front::Disabled).await;
        fx.exit_on.store(true, std::sync::atomic::Ordering::Relaxed);

        let said = fx.when_socks().await;

        assert_eq!(said, ["listening: off; exit for others: on"]);
    }
}

#[derive(Clone, Copy)]
enum Path {
    Direct,
    OfflineExit,
}

enum Front {
    Idle,
    Disabled,
    Targeting(Path),
}

struct Fixture {
    front: ProxyFront,
    console: ProxyConsole,
    exit_on: Arc<AtomicBool>,
    node: crate::node::RunningNode,
}

async fn setup(front: Front) -> Fixture {
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(
        crate::members::network_key::NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    let settings = ProcessSettings {
        socks_port: 0,
        socks_enabled: !matches!(front, Front::Disabled),
        ..ProcessSettings::default()
    };
    let bound = ProxyFront::bind(&settings).await.expect("a free port");
    let exit_on = Arc::new(AtomicBool::new(false));
    let console = ProxyConsole {
        front: Some(bound.clone()),
        exit_on: exit_on.clone(),
        user_agent: "meshtown-test".to_string(),
    };
    let fx = Fixture {
        front: bound,
        console,
        exit_on,
        node,
    };
    if let Front::Targeting(path) = front {
        fx.front.set_target(Some(fx.config(path)));
    }
    fx
}

impl Fixture {
    /// A proxy configuration whose route leads `path`.
    fn config(&self, path: Path) -> ProxyConfig {
        let resolver = Resolver::new(
            SocketAddr::from(([127, 0, 0, 1], 1)),
            HttpLimits::default(),
            "meshtown-test".to_string(),
        )
        .expect("resolver with a stub address");
        ProxyConfig {
            route: route_of(&self.node, path),
            stack: self.node.stack.clone(),
            pool: PeerConnPool::default(),
            resolver: Arc::new(resolver),
            host_overrides: Arc::new(Default::default()),
            udp_slots: crate::proxy::udp_slots(),
        }
    }

    /// Runs `route <domain>` and returns the lines the console said about that domain.
    async fn when_route(&self, domain: &str) -> Vec<String> {
        let before = log_summary(0).last_seq;
        execute_proxy_command(&self.console, ProxyCommand::Route(domain.to_string())).await;
        said_since(before, "route", domain)
    }

    /// Runs `socks` and returns the `listening:` lines said since; the tests make the address or the exit flag unique.
    async fn when_socks(&self) -> Vec<String> {
        let before = log_summary(0).last_seq;
        execute_proxy_command(&self.console, ProxyCommand::Socks).await;
        let wanted = match self.front.addr() {
            Some(addr) => format!("listening: {addr};"),
            None => "listening: off; exit for others: on".to_string(),
        };
        said_since(before, "socks", &wanted)
    }
}

fn route_of(node: &crate::node::RunningNode, path: Path) -> RouteCtx {
    let exits = match path {
        Path::Direct => Vec::new(),
        Path::OfflineExit => vec![offline_peer()],
    };
    RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, exits)
}

/// A peer that is not a member of the node's network.
fn offline_peer() -> PeerId {
    NodeSecret::from_bytes(&[91; 32]).peer_id()
}

/// Texts of the lines with `tag` said after `seq` that contain `needle`.
fn said_since(seq: u64, tag: &str, needle: &str) -> Vec<String> {
    log_page(seq, 10_000)
        .lines
        .into_iter()
        .filter(|l| l.tag == tag && l.text.contains(needle))
        .map(|l| l.text)
        .collect()
}
