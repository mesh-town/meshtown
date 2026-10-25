//! Tests for the mesh branch of `open_target` (`mesh/open.rs`): a `.meshtown` host is answered by the node itself,
//! never sent to an exit and never resolved; every other host keeps the ordinary route.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::exit::PeerConnPool;
use crate::proxy::front::NetDirectory;
use crate::proxy::mesh::host::MESH_SUFFIX;
use crate::proxy::mesh::open::open_target;
use crate::proxy::path::relay_bytes;
use crate::proxy::route::{Route, RouteCtx};
use crate::proxy::socks5::Target;
use crate::proxy::ProxyConfig;
use crate::site_checks::doh::Resolver;
use crate::site_checks::http_min::HttpLimits;
use crate::transport::net::facade::test_support::{pid, within};

const SITE_BODY: &str = "hello from the site";
const GHOST_GET: &[u8] = b"GET / HTTP/1.1\r\nHost: ghost.meshtown\r\n\r\n";
const SITE_GET: &[u8] = b"GET / HTTP/1.1\r\nHost: site.test\r\n\r\n";
const TLS_HELLO_START: &[u8] = &[0x16, 0x03, 0x01];
const NO_CONNECTION_WINDOW: Duration = Duration::from_millis(300);

mod host_in_the_mesh_suffix {
    use super::*;

    mod with_a_live_address_in_the_overrides {
        use super::*;

        #[tokio::test]
        async fn is_answered_with_a_404_page() {
            // given
            let trap = Trap::bind().await;
            let t = setup(trap.overrides("ghost.meshtown")).await;

            // when
            let answer = t.fetch("ghost.meshtown", 80, GHOST_GET).await;

            // then
            assert_status(&answer, "404");
        }

        #[tokio::test]
        async fn does_not_connect_to_the_overridden_address() {
            // given
            let trap = Trap::bind().await;
            let t = setup(trap.overrides("ghost.meshtown")).await;

            // when
            t.fetch("ghost.meshtown", 80, GHOST_GET).await;

            // then
            assert!(
                trap.accepted_nothing().await,
                "a mesh host must not be resolved or connected to"
            );
        }
    }

    mod when_the_chosen_exits_are_offline {
        use super::*;

        #[tokio::test]
        async fn is_opened_as_a_404_page_not_refused() {
            // given
            let t = setup(HashMap::new()).await;
            t.cfg.route.set_chosen_exits(vec![pid(9)]);
            assert!(
                matches!(t.cfg.route.choose_now().await, Ok(Route::Refuse(_))),
                "the precondition: the route refuses"
            );

            // when
            let answer = t.fetch("ghost.meshtown", 80, GHOST_GET).await;

            // then
            assert_status(&answer, "404");
        }
    }

    mod while_the_own_name_is_not_in_the_document {
        use super::*;

        #[tokio::test]
        async fn page_calls_this_machine_this_computer() {
            // given
            let t = setup(HashMap::new()).await;

            // when
            let answer = t.fetch("ghost.meshtown", 80, GHOST_GET).await;

            // then
            let text = String::from_utf8_lossy(&answer);
            assert!(text.contains("on this computer"), "{text}");
        }
    }

    mod with_a_nested_label {
        use super::*;

        #[tokio::test]
        async fn is_answered_with_a_404_page() {
            // given
            let t = setup(HashMap::new()).await;

            // when
            let answer = t.fetch("a.b.meshtown", 80, GHOST_GET).await;

            // then
            assert_status(&answer, "404");
        }
    }

    mod on_port_443_with_a_tls_hello {
        use super::*;

        #[tokio::test]
        async fn ends_with_eof_and_no_bytes() {
            // given
            let t = setup(HashMap::new()).await;

            // when
            let answer = t.fetch("ghost.meshtown", 443, TLS_HELLO_START).await;

            // then
            assert!(answer.is_empty(), "TLS must get no answer, got {answer:?}");
        }
    }
}

mod ordinary_host {
    use super::*;

    #[tokio::test]
    async fn goes_the_direct_path_to_the_site() {
        // given
        let site = Site::start().await;
        let t = setup(HashMap::from([("site.test".to_string(), site.addr)])).await;

        // when
        let answer = t.fetch("site.test", 80, SITE_GET).await;

        // then
        let text = String::from_utf8_lossy(&answer);
        assert!(
            text.contains(SITE_BODY),
            "the site's answer is expected: {text}"
        );
        assert_eq!(site.hits().await, 1, "the site must be contacted once");
    }
}

mod pac_script {
    use super::*;

    #[test]
    fn routes_the_same_suffix_as_the_proxy_answers() {
        // given
        let pac = include_str!("../pac.js");

        // when
        let suffix = format!(".{MESH_SUFFIX}");

        // then
        assert!(
            pac.contains(&suffix),
            "pac.js must send `{suffix}` hosts to the node's proxy"
        );
    }
}

struct Fixture {
    cfg: ProxyConfig,
    nets: NetDirectory,
}

/// A proxy config on a loopback node with an empty network directory.
async fn setup(hosts: HashMap<String, SocketAddr>) -> Fixture {
    Fixture {
        cfg: mesh_config(hosts).await,
        nets: NetDirectory::default(),
    }
}

impl Fixture {
    /// `open_target` for `host:port`, then the client sends `request` and reads the answer until EOF.
    async fn fetch(&self, host: &str, port: u16, request: &[u8]) -> Vec<u8> {
        let target = Target::Domain(host.to_string());
        let opened = within(
            "open_target",
            open_target(&self.cfg, &self.nets, &target, port, "test"),
        )
        .await
        .ok()
        .expect("open_target must open the target");
        let (mut client, server) = tcp_pair().await;
        let route = self.cfg.route.clone();
        let request = request.to_vec();
        crate::transport::net::task_label::spawn(async move {
            relay_bytes(opened, &route, server, &request).await
        });
        let mut answer = Vec::new();
        within("read the answer", client.read_to_end(&mut answer))
            .await
            .expect("the answer is read to EOF");
        answer
    }
}

fn assert_status(answer: &[u8], code: &str) {
    let want = format!("HTTP/1.1 {code}");
    assert!(
        answer.starts_with(want.as_bytes()),
        "expected `{want}`, got {:?}",
        String::from_utf8_lossy(&answer[..answer.len().min(40)])
    );
}

/// A proxy config with a dead resolver and a `host_overrides` table; the node is real, on loopback.
async fn mesh_config(hosts: HashMap<String, SocketAddr>) -> ProxyConfig {
    let key = crate::members::network_key::NetworkKey::generate().unwrap();
    let node = crate::node::start_node(crate::node::NodeConfig::for_loopback_test(key))
        .await
        .unwrap();
    let resolver = Resolver::new(
        SocketAddr::from(([127, 0, 0, 1], 1)),
        HttpLimits::default(),
        "meshtown-test".to_string(),
    )
    .expect("resolver with a stub address");
    ProxyConfig {
        route: RouteCtx::new(node.members.clone(), node.domains.clone(), node.id, vec![]),
        stack: node.stack.clone(),
        pool: PeerConnPool::default(),
        resolver: Arc::new(resolver),
        host_overrides: Arc::new(hosts),
        udp_slots: crate::proxy::udp_slots(),
    }
}

/// A connected loopback pair: (the test's client side, the proxy's accepted side).
async fn tcp_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
    (client.unwrap(), accepted.unwrap().0)
}

/// A live listener that must never be connected to.
struct Trap {
    listener: TcpListener,
}

impl Trap {
    async fn bind() -> Trap {
        Trap {
            listener: TcpListener::bind("127.0.0.1:0").await.unwrap(),
        }
    }

    fn overrides(&self, host: &str) -> HashMap<String, SocketAddr> {
        HashMap::from([(host.to_string(), self.listener.local_addr().unwrap())])
    }

    async fn accepted_nothing(&self) -> bool {
        tokio::time::timeout(NO_CONNECTION_WINDOW, self.listener.accept())
            .await
            .is_err()
    }
}

/// A site that answers every connection with one fixed response and counts them.
struct Site {
    addr: SocketAddr,
    hits: Arc<tokio::sync::Mutex<u32>>,
}

impl Site {
    async fn start() -> Site {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(tokio::sync::Mutex::new(0u32));
        crate::transport::net::task_label::spawn(serve_site(listener, hits.clone()));
        Site { addr, hits }
    }

    async fn hits(&self) -> u32 {
        *self.hits.lock().await
    }
}

async fn serve_site(listener: TcpListener, hits: Arc<tokio::sync::Mutex<u32>>) {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        *hits.lock().await += 1;
        crate::transport::net::task_label::spawn(async move {
            let mut head = [0u8; 1024];
            let _ = stream.read(&mut head).await;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{SITE_BODY}",
                SITE_BODY.len()
            );
            let _ = stream.write_all(reply.as_bytes()).await;
            let _ = stream.shutdown().await;
        });
    }
}
