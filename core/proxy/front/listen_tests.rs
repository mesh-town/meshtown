//! Tests of the process-wide proxy listener: taking the port, refusing without a target, switching the
//! target, re-applying the settings and closing.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use super::*;
use crate::config::config_file::{IfaceFilter, ListenAccess, ProcessSettings};
use crate::exit::PeerConnPool;
use crate::members::network_key::NetworkKey;
use crate::node::{start_node, NodeConfig, RunningNode};
use crate::proxy::socks5::{
    encode_socks5_request, encode_udp_associate_request, Target, REP_GENERAL, REP_HOST_UNREACHABLE,
    REP_OK,
};
use crate::proxy::{udp_slots, BindError, ProxyConfig};
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;

mod bind {
    use super::*;

    mod busy_port {
        use super::*;

        #[tokio::test]
        async fn fails_with_bind_error_naming_the_port() {
            // given: another program holds the port
            let foreign = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = foreign.local_addr().unwrap().port();

            // when: the front binds it
            let err = bind_front(&settings(port)).await.err().expect("bind fails");

            // then: the error is a `BindError` about that port and an address in use
            let bind_error = err.downcast_ref::<BindError>().expect("a BindError");
            assert_eq!(bind_error.port, port);
            assert_eq!(bind_error.io_error_kind, std::io::ErrorKind::AddrInUse);
        }

        #[tokio::test]
        async fn error_text_and_advice_are_the_usual_ones() {
            // given: another program holds the port
            let foreign = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = foreign.local_addr().unwrap().port();

            // when: the front binds it
            let err = bind_front(&settings(port)).await.err().expect("bind fails");

            // then: the text and the advice are those of the old listener
            let bind_error = err.downcast_ref::<BindError>().unwrap();
            assert!(
                bind_error.to_string().starts_with(&format!(
                    "port {port} for the browser proxy is in use by another program ("
                )),
                "text: {bind_error}"
            );
            assert!(
                bind_error
                    .advice()
                    .starts_with("Close it or start Mesh Town on another port: "),
                "advice: {}",
                bind_error.advice()
            );
            assert!(
                bind_error.advice().contains("run --socks-port "),
                "advice: {}",
                bind_error.advice()
            );
        }
    }

    mod proxy_disabled {
        use super::*;

        #[tokio::test]
        async fn starts_without_a_listener() {
            // given: `socks.enabled` is false
            let off = ProcessSettings {
                socks_enabled: false,
                ..settings(free_port())
            };

            // when: the front binds
            let front = bind_front(&off).await.expect("bind succeeds");

            // then: there is no address, neither in the front nor in its face
            assert_eq!(front.addr(), None);
            assert_eq!(front.face().borrow().addr, None);
        }

        #[tokio::test]
        async fn leaves_the_port_to_others() {
            // given: the proxy is off for a port
            let port = free_port();
            let off = ProcessSettings {
                socks_enabled: false,
                ..settings(port)
            };

            // when: the front binds
            let _front = bind_front(&off).await.unwrap();

            // then: the port is free
            assert!(can_bind(port), "port {port} is taken by a disabled front");
        }
    }

    mod port_zero {
        use super::*;

        #[tokio::test]
        async fn picks_a_free_port_on_loopback() {
            // given/when: the port is 0
            let front = bind_front(&settings(0)).await.unwrap();

            // then: the address is loopback with the port the OS picked
            let addr = front.addr().expect("the proxy listens");
            assert!(addr.ip().is_loopback(), "address {addr}");
            assert_ne!(addr.port(), 0, "the real port is reported");
        }
    }

    mod localnet {
        use super::*;

        #[tokio::test]
        async fn reports_loopback_as_the_address() {
            // given/when: `localnet` on a free port
            let port = free_port();
            let front = bind_front(&localnet_settings(port)).await.unwrap();

            // then: the address stays `127.0.0.1:<port>` and the face says `localnet`
            assert_eq!(front.addr(), Some(loopback(port)));
            assert_eq!(front.face().borrow().access, ListenAccess::Localnet);
        }

        #[tokio::test]
        async fn serves_a_loopback_client() {
            // given: `localnet` mode
            let front = bind_front(&localnet_settings(free_port())).await.unwrap();

            // when: a loopback client greets
            let reply = greet(front.addr().unwrap()).await;

            // then: the greeting is accepted
            assert_eq!(reply, [5, 0]);
        }

        #[tokio::test]
        async fn listens_on_the_lan_address() {
            // given: `localnet` mode (a host without a LAN address has nothing to check)
            let Some(lan) = lan_ip() else { return };
            let port = free_port();
            let _front = bind_front(&localnet_settings(port)).await.unwrap();

            // when: a client connects to the LAN address
            let connected = TcpStream::connect((lan, port)).await;

            // then: the listener answers on all interfaces
            assert!(connected.is_ok(), "{lan}:{port} is not listening");
        }
    }

    mod localhost {
        use super::*;

        #[tokio::test]
        async fn does_not_listen_on_the_lan_address() {
            // given: `localhost` mode (a host without a LAN address has nothing to check)
            let Some(lan) = lan_ip() else { return };
            let port = free_port();
            let _front = bind_front(&settings(port)).await.unwrap();

            // when: a client connects to the LAN address
            let connected = TcpStream::connect((lan, port)).await;

            // then: nobody answers there
            assert!(connected.is_err(), "{lan}:{port} is listening");
        }
    }
}

mod no_target {
    use super::*;

    #[tokio::test]
    async fn socks_connect_gets_host_unreachable() {
        // given: a front without a target
        let w = setup(Opts::default()).await;

        // when: a client asks for CONNECT
        let (_stream, reply) = socks_exchange(w.addr(), connect_request()).await;

        // then: the reply code is 4
        assert_eq!(reply, REP_HOST_UNREACHABLE);
    }

    #[tokio::test]
    async fn udp_associate_gets_host_unreachable() {
        // given: a front without a target
        let w = setup(Opts::default()).await;

        // when: a client asks for UDP ASSOCIATE
        let (_stream, reply) = socks_exchange(w.addr(), encode_udp_associate_request()).await;

        // then: the reply code is 4
        assert_eq!(reply, REP_HOST_UNREACHABLE);
    }

    #[tokio::test]
    async fn http_connect_gets_502_with_the_reason() {
        // given: a front without a target
        let w = setup(Opts::default()).await;

        // when: a client sends HTTP CONNECT
        let answer = http_connect(w.addr()).await;

        // then: 502 and the reason in the body
        assert!(answer.starts_with("HTTP/1.1 502"), "answer: {answer}");
        assert!(
            answer.contains("no network for the proxy"),
            "answer: {answer}"
        );
    }

    #[tokio::test]
    async fn pac_is_served_with_the_listener_address() {
        // given: a front without a target
        let w = setup(Opts::default()).await;

        // when: a client asks for the PAC
        let answer = http_pac(w.addr()).await;

        // then: 200 and the script names the proxy address
        assert!(answer.starts_with("HTTP/1.1 200"), "answer: {answer}");
        assert!(answer.contains(&w.addr().to_string()), "answer: {answer}");
    }

    #[tokio::test]
    async fn current_route_is_none() {
        // given/when: a front without a target
        let w = setup(Opts::default()).await;

        // then: there is no route to show
        assert!(w.front.current_route().is_none());
    }
}

mod target {
    use super::*;

    #[tokio::test]
    async fn direct_target_carries_traffic_to_the_echo() {
        // given: a front whose target has a direct path to the echo
        let w = setup(Opts::to(Aim::Direct)).await;

        // when: a client connects through it
        let mut client = w.connect().await;

        // then: the echo answers
        assert_echoes(&mut client).await;
    }

    #[tokio::test]
    async fn current_route_follows_the_target() {
        // given: a front with a target
        let w = setup(Opts::to(Aim::Direct)).await;
        assert!(w.front.current_route().is_some(), "a target is set");

        // when: the target is cleared
        w.front.set_target(None);

        // then: there is no route
        assert!(w.front.current_route().is_none());
    }

    mod switched_to_offline_exits {
        use super::*;

        async fn switched() -> (World, TcpStream) {
            // given: connection 1 is open through the direct target
            let w = setup(Opts::to(Aim::Direct)).await;
            let mut first = w.connect().await;
            assert_echoes(&mut first).await;
            // when: the target becomes a network whose chosen exits are offline
            w.front.set_target(Some(w.offline.clone()));
            (w, first)
        }

        #[tokio::test]
        async fn open_connection_keeps_working() {
            let (_w, mut first) = switched().await;

            assert_echoes(&mut first).await;
        }

        #[tokio::test]
        async fn new_socks_connection_is_refused() {
            let (w, _first) = switched().await;

            let (_stream, reply) = socks_exchange(w.addr(), connect_request()).await;

            assert_eq!(reply, REP_HOST_UNREACHABLE);
        }

        #[tokio::test]
        async fn new_http_connection_is_refused_for_offline_exits() {
            let (w, _first) = switched().await;

            let answer = http_connect(w.addr()).await;

            assert!(answer.starts_with("HTTP/1.1 502"), "answer: {answer}");
            assert!(
                answer.contains("chosen exits are offline"),
                "answer: {answer}"
            );
        }
    }

    mod cleared {
        use super::*;

        #[tokio::test]
        async fn new_connections_are_refused_and_open_one_survives() {
            // given: connection 1 is open through the direct target
            let w = setup(Opts::to(Aim::Direct)).await;
            let mut first = w.connect().await;
            assert_echoes(&mut first).await;

            // when: the target is cleared
            w.front.set_target(None);

            // then: a new connection is refused, connection 1 works
            let (_stream, reply) = socks_exchange(w.addr(), connect_request()).await;
            assert_eq!(reply, REP_HOST_UNREACHABLE);
            assert_echoes(&mut first).await;
        }

        #[tokio::test]
        async fn a_new_target_serves_again() {
            // given: a front whose target was cleared
            let w = setup(Opts::to(Aim::Direct)).await;
            w.front.set_target(None);

            // when: the target is set again
            w.front.set_target(Some(w.direct.clone()));

            // then: a new connection reaches the echo
            let mut client = w.connect().await;
            assert_echoes(&mut client).await;
        }
    }

    mod udp_slots {
        use super::*;

        #[tokio::test]
        async fn target_slots_are_replaced_by_the_process_ones() {
            // given: a target whose own UDP slots are exhausted
            let w = setup(Opts::default()).await;
            let exhausted = ProxyConfig {
                udp_slots: Arc::new(Semaphore::new(0)),
                ..w.direct.clone()
            };

            // when: it becomes the target and a client asks for UDP ASSOCIATE
            w.front.set_target(Some(exhausted));
            let (_stream, reply) = socks_exchange(w.addr(), encode_udp_associate_request()).await;

            // then: the association is granted from the process slots, not refused for "too many"
            assert_ne!(reply, REP_GENERAL, "the target's own slots were used");
            assert_eq!(reply, REP_OK);
        }

        #[tokio::test]
        async fn slots_stay_in_force_after_the_target_changes() {
            // given: two targets, both with exhausted own UDP slots
            let w = setup(Opts::default()).await;
            let exhausted = |cfg: &ProxyConfig| ProxyConfig {
                udp_slots: Arc::new(Semaphore::new(0)),
                ..cfg.clone()
            };
            w.front.set_target(Some(exhausted(&w.direct)));

            // when: the target is replaced by another one
            w.front.set_target(Some(exhausted(&w.direct)));

            // then: UDP ASSOCIATE is still granted
            let (_stream, reply) = socks_exchange(w.addr(), encode_udp_associate_request()).await;
            assert_eq!(reply, REP_OK);
        }
    }
}

mod apply {
    use super::*;

    mod same_settings {
        use super::*;

        #[tokio::test]
        async fn keeps_the_address_and_the_open_connection() {
            // given: connection 1 is open
            let w = setup(Opts::to(Aim::Direct)).await;
            let addr = w.addr();
            let mut first = w.connect().await;
            assert_echoes(&mut first).await;

            // when: the same settings are applied
            w.front.apply(&w.settings).await.unwrap();

            // then: the address is the same, connection 1 works, a new one is served
            assert_eq!(w.front.addr(), Some(addr));
            assert_echoes(&mut first).await;
            assert_echoes(&mut w.connect().await).await;
        }
    }

    mod another_port {
        use super::*;

        #[tokio::test]
        async fn moves_the_listener_and_frees_the_old_port() {
            // given: a front on port P1
            let w = setup(Opts::to(Aim::Direct)).await;
            let old = w.addr().port();
            let new = free_port();

            // when: port P2 is applied
            w.front.apply(&settings(new)).await.unwrap();

            // then: the front listens on P2 and P1 is free
            assert_eq!(w.front.addr(), Some(loopback(new)));
            assert!(can_bind(old), "the old port {old} is still taken");
            assert_echoes(&mut w.connect().await).await;
        }

        #[tokio::test]
        async fn publishes_the_new_address_in_the_face() {
            let w = setup(Opts::default()).await;
            let face = w.front.face();
            let new = free_port();

            w.front.apply(&settings(new)).await.unwrap();

            assert_eq!(face.borrow().addr, Some(loopback(new)));
        }

        #[tokio::test]
        async fn keeps_the_open_connection() {
            let w = setup(Opts::to(Aim::Direct)).await;
            let mut first = w.connect().await;
            assert_echoes(&mut first).await;

            w.front.apply(&settings(free_port())).await.unwrap();

            assert_echoes(&mut first).await;
        }
    }

    mod busy_port {
        use super::*;

        async fn refused() -> (World, u16, anyhow::Error) {
            // given: a front on P1 and another program on P2
            let w = setup(Opts::to(Aim::Direct)).await;
            let foreign = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let busy = foreign.local_addr().unwrap().port();
            // when: P2 is applied
            let err = w
                .front
                .apply(&settings(busy))
                .await
                .expect_err("P2 is busy");
            // the foreign listener lives until the end of the test
            std::mem::forget(foreign);
            (w, busy, err)
        }

        #[tokio::test]
        async fn fails_with_bind_error() {
            let (_w, busy, err) = refused().await;

            let bind_error = err.downcast_ref::<BindError>().expect("a BindError");
            assert_eq!(bind_error.port, busy);
        }

        #[tokio::test]
        async fn old_listener_keeps_serving() {
            let (w, _busy, _err) = refused().await;

            assert!(w.front.addr().is_some(), "the address is gone");
            assert_echoes(&mut w.connect().await).await;
        }
    }

    mod same_port_other_access {
        use super::*;

        async fn widened() -> World {
            // given: a `localhost` front
            let w = setup(Opts::to(Aim::Direct)).await;
            // when: the same port is applied with `localnet`
            let widened = ProcessSettings {
                socks_access: ListenAccess::Localnet,
                ..w.settings.clone()
            };
            w.front.apply(&widened).await.unwrap();
            w
        }

        #[tokio::test]
        async fn keeps_the_address_and_takes_the_new_rule() {
            let w = widened().await;

            assert_eq!(w.front.addr(), Some(loopback(w.port)));
            assert_eq!(w.front.face().borrow().access, ListenAccess::Localnet);
        }

        #[tokio::test]
        async fn serves_a_new_client() {
            let w = widened().await;

            assert_echoes(&mut w.connect().await).await;
        }

        #[tokio::test]
        async fn listens_on_the_lan_address_now() {
            // a host without a LAN address has nothing to check
            let Some(lan) = lan_ip() else { return };
            let w = widened().await;

            let connected = TcpStream::connect((lan, w.port)).await;

            assert!(connected.is_ok(), "{lan}:{} is not listening", w.port);
        }
    }

    mod same_port_other_ifaces {
        use super::*;

        #[tokio::test]
        async fn takes_the_new_filter() {
            // given: a front with automatic interfaces
            let w = setup(Opts::to(Aim::Direct)).await;
            let only = IfaceFilter::Only(vec!["no-such0".to_string()]);

            // when: the same port is applied with another filter
            let changed = ProcessSettings {
                socks_ifaces: only.clone(),
                ..w.settings.clone()
            };
            w.front.apply(&changed).await.unwrap();

            // then: the face shows the filter, the address is the same
            assert_eq!(w.front.face().borrow().ifaces, only);
            assert_eq!(w.front.addr(), Some(loopback(w.port)));
        }
    }

    mod turned_off {
        use super::*;

        #[tokio::test]
        async fn drops_the_address_and_frees_the_port() {
            // given: a front on a port
            let w = setup(Opts::to(Aim::Direct)).await;

            // when: the proxy is switched off
            let off = ProcessSettings {
                socks_enabled: false,
                ..w.settings.clone()
            };
            w.front.apply(&off).await.unwrap();

            // then: no address, the port is free
            assert_eq!(w.front.addr(), None);
            assert!(can_bind(w.port), "port {} is still taken", w.port);
        }
    }

    mod turned_on {
        use super::*;

        #[tokio::test]
        async fn starts_listening() {
            // given: a front bound with the proxy off
            let port = free_port();
            let off = ProcessSettings {
                socks_enabled: false,
                ..settings(port)
            };
            let front = bind_front(&off).await.unwrap();

            // when: the proxy is switched on
            front.apply(&settings(port)).await.unwrap();

            // then: it listens on the port
            assert_eq!(front.addr(), Some(loopback(port)));
            assert_eq!(greet(loopback(port)).await, [5, 0]);
        }
    }

    mod concurrent_calls {
        use super::*;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn two_applies_on_different_ports_leave_one_listener() {
            // given: a front on P0 and two other ports
            let w = setup(Opts::default()).await;
            let (p1, p2) = (free_port(), free_port());

            // when: both are applied at once
            let (for_p1, for_p2) = (settings(p1), settings(p2));
            let (first, second) = tokio::join!(w.front.apply(&for_p1), w.front.apply(&for_p2));
            first.unwrap();
            second.unwrap();

            // then: exactly one of them listens (the address is the front's), the others are free
            let addr = w.front.addr().expect("the proxy listens");
            let listening: Vec<u16> = [w.port, p1, p2]
                .into_iter()
                .filter(|p| !can_bind(*p))
                .collect();
            assert_eq!(listening, vec![addr.port()], "listening ports");
            assert!(
                [p1, p2].contains(&addr.port()),
                "the address {addr} is not from the applied settings"
            );
        }
    }
}

mod close {
    use super::*;

    #[tokio::test]
    async fn frees_the_port_at_once() {
        // given: a front on a fixed port
        let w = setup(Opts::default()).await;

        // when: it is closed
        w.front.close().await;

        // then: the port is free when `close` returns
        assert!(can_bind(w.port), "port {} is busy after close", w.port);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_same_port_is_taken_again_twenty_times() {
        let port = free_port();
        for round in 0..20 {
            // given/when: bind and close on one port, round after round
            let front = bind_front(&settings(port)).await.unwrap_or_else(|e| {
                panic!("round {round}, port {port}: {e:#}");
            });
            assert_eq!(front.addr(), Some(loopback(port)), "round {round}");
            front.close().await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn foreign_listener_after_close_fails_the_next_bind_with_the_same_text() {
        // given: a front was closed and another program took its port
        let w = setup(Opts::default()).await;
        w.front.close().await;
        let _foreign = std::net::TcpListener::bind(("127.0.0.1", w.port)).unwrap();

        // when: a front binds the port again
        let err = bind_front(&w.settings)
            .await
            .err()
            .expect("the port is busy");

        // then: the error is the usual one
        let text = format!("{err:#}");
        assert!(
            text.contains(&format!(
                "port {} for the browser proxy is in use by another program",
                w.port
            )),
            "text: {text}"
        );
        assert!(err.downcast_ref::<BindError>().is_some(), "a BindError");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn open_client_does_not_hold_the_port() {
        // given: a client has greeted and stays connected
        let w = setup(Opts::default()).await;
        let mut client = TcpStream::connect(w.addr()).await.unwrap();
        client.write_all(&[5, 1, 0]).await.unwrap();
        let mut reply = [0u8; 2];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply, [5, 0]);

        // when: the front is closed
        w.front.close().await;

        // then: the port is free while the client is still open
        assert!(can_bind(w.port), "port {} is held by a client", w.port);
    }

    #[tokio::test]
    async fn second_close_through_a_clone_returns() {
        // given: a front closed through one handle
        let w = setup(Opts::default()).await;
        let clone = w.front.clone();
        w.front.close().await;

        // when/then: closing through the clone returns
        assert!(timeout(LIMIT, clone.close()).await.is_ok(), "close hangs");
    }

    #[tokio::test]
    async fn close_of_a_disabled_front_returns() {
        // given: a front with the proxy off
        let off = ProcessSettings {
            socks_enabled: false,
            ..settings(free_port())
        };
        let front = bind_front(&off).await.unwrap();

        // when/then: it closes without waiting for anything
        assert!(timeout(LIMIT, front.close()).await.is_ok(), "close hangs");
    }
}

const LIMIT: Duration = Duration::from_secs(5);
const ECHO_NAME: &str = "echo.test";

/// Which target the fixture's front starts with.
#[derive(Clone, Copy, Default)]
enum Aim {
    #[default]
    Nothing,
    Direct,
}

#[derive(Default)]
struct Opts {
    aim: Aim,
}

impl Opts {
    fn to(aim: Aim) -> Opts {
        Opts { aim }
    }
}

/// A front on a free fixed port plus two target configurations: `direct` leads to the echo site,
/// `offline` has a chosen exit that is not online.
struct World {
    front: ProxyFront,
    port: u16,
    settings: ProcessSettings,
    direct: ProxyConfig,
    offline: ProxyConfig,
    _nodes: [RunningNode; 2],
}

async fn setup(opts: Opts) -> World {
    let echo = echo_site().await;
    let (direct_node, offline_node) = two_nodes().await;
    let overrides = Arc::new(HashMap::from([(ECHO_NAME.to_string(), echo)]));
    let (direct, offline) = (
        config_of(&direct_node, &overrides),
        config_of(&offline_node, &overrides),
    );
    let port = free_port();
    let settings = settings(port);
    let front = bind_front(&settings).await.unwrap();
    match opts.aim {
        Aim::Nothing => {}
        Aim::Direct => front.set_target(Some(direct.clone())),
    }
    World {
        front,
        port,
        settings,
        direct,
        offline,
        _nodes: [direct_node, offline_node],
    }
}

/// Two loopback nodes in different networks; the second one has the first one as its only chosen
/// exit, and it is not a member there — an offline exit.
async fn two_nodes() -> (RunningNode, RunningNode) {
    let first = start_node(NodeConfig::for_loopback_test(
        NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    let second = start_node(NodeConfig::for_loopback_test(
        NetworkKey::generate().unwrap(),
    ))
    .await
    .unwrap();
    second.set_route_exits(vec![first.id]);
    (first, second)
}

/// The proxy configuration of a node; the echo name leads to the echo site.
fn config_of(node: &RunningNode, overrides: &Arc<HashMap<String, SocketAddr>>) -> ProxyConfig {
    let silent_doh = SocketAddr::from(([127, 0, 0, 1], 1));
    let resolver = Resolver::new(
        silent_doh,
        HttpLimits::default(),
        "meshtown-test".to_string(),
    );
    ProxyConfig {
        route: node.route.clone(),
        stack: node.stack.clone(),
        pool: PeerConnPool::default(),
        resolver: Arc::new(resolver.expect("a resolver with a stub address")),
        host_overrides: overrides.clone(),
        udp_slots: udp_slots(),
    }
}

impl World {
    fn addr(&self) -> SocketAddr {
        self.front.addr().expect("the proxy listens")
    }

    /// A client connection to the echo site through the front; panics if the front refuses it.
    async fn connect(&self) -> TcpStream {
        let (stream, reply) = socks_exchange(self.addr(), connect_request()).await;
        assert_eq!(reply, REP_OK, "the front refused the connection");
        stream
    }
}

async fn bind_front(settings: &ProcessSettings) -> anyhow::Result<ProxyFront> {
    ProxyFront::bind(settings).await
}

fn settings(port: u16) -> ProcessSettings {
    ProcessSettings {
        socks_port: port,
        ..ProcessSettings::default()
    }
}

fn localnet_settings(port: u16) -> ProcessSettings {
    ProcessSettings {
        socks_access: ListenAccess::Localnet,
        ..settings(port)
    }
}

fn loopback(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// A free port on loopback: bind with port zero, remember it, close.
fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn can_bind(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// This machine's address for the local network, if it has one.
fn lan_ip() -> Option<std::net::Ipv4Addr> {
    crate::web_gui::auth::access::lan_addrs()
        .first()
        .map(|(_, ip)| *ip)
}

/// A TCP server that sends back what it reads.
async fn echo_site() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    crate::node::output::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            crate::node::output::spawn(async move {
                let mut buf = [0u8; 1024];
                while let Ok(n) = socket.read(&mut buf).await {
                    if n == 0 || socket.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    addr
}

fn connect_request() -> Vec<u8> {
    encode_socks5_request(&Target::Domain(ECHO_NAME.to_string()), 80).unwrap()
}

/// Opens a connection, greets, sends the request; returns the stream and the reply code
/// (the reply is read whole, so the stream is ready for data after a success).
async fn socks_exchange(proxy: SocketAddr, request: Vec<u8>) -> (TcpStream, u8) {
    let mut stream = TcpStream::connect(proxy).await.unwrap();
    stream.write_all(&[5, 1, 0]).await.unwrap();
    let mut method = [0u8; 2];
    stream.read_exact(&mut method).await.unwrap();
    stream.write_all(&request).await.unwrap();
    let mut reply = [0u8; 10];
    timeout(LIMIT, stream.read_exact(&mut reply))
        .await
        .expect("no reply within the limit")
        .unwrap();
    (stream, reply[1])
}

/// Sends the SOCKS5 greeting on a new connection and returns the two reply bytes.
async fn greet(proxy: SocketAddr) -> [u8; 2] {
    let mut stream = TcpStream::connect(proxy).await.unwrap();
    stream.write_all(&[5, 1, 0]).await.unwrap();
    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).await.unwrap();
    reply
}

/// HTTP `CONNECT` to the echo name; returns the whole answer of a refusal (the proxy closes the connection).
async fn http_connect(proxy: SocketAddr) -> String {
    let request = format!("CONNECT {ECHO_NAME}:80 HTTP/1.1\r\nHost: {ECHO_NAME}:80\r\n\r\n");
    http_exchange(proxy, &request).await
}

async fn http_pac(proxy: SocketAddr) -> String {
    let request = format!("GET /proxy.pac HTTP/1.1\r\nHost: {proxy}\r\n\r\n");
    http_exchange(proxy, &request).await
}

async fn http_exchange(proxy: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(proxy).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = Vec::new();
    timeout(LIMIT, stream.read_to_end(&mut answer))
        .await
        .expect("the proxy did not close the connection")
        .unwrap();
    String::from_utf8_lossy(&answer).into_owned()
}

/// Sends a line through the tunnel and expects the same bytes back.
async fn assert_echoes(stream: &mut TcpStream) {
    stream.write_all(b"ping").await.unwrap();
    let mut back = [0u8; 4];
    timeout(LIMIT, stream.read_exact(&mut back))
        .await
        .expect("no echo within the limit")
        .unwrap();
    assert_eq!(&back, b"ping", "the echo returned other bytes");
}
