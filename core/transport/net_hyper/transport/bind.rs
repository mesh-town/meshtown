//! Starting the transport: the DHT node, the server, the gate and the background tasks.

use super::factory::factory_for;
use super::hints::HintBook;
use super::relay::RelayDial;
use super::{
    dht_error_to_net_error, run_listen, HyperOpts, HyperTransport, NodeDialer, ACCEPT_QUEUE,
    DIAL_SLOTS,
};
use crate::transport::net::error::NetResult;
use crate::transport::net::id::NodeSecret;
use crate::transport::net_hyper::accept::{accept_loop, AcceptCtx};
use crate::transport::net_hyper::dht_table::TableSaver;
use crate::transport::net_hyper::gate::PeerGate;
use crate::transport::net_hyper::keys::key_pair;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, Semaphore};
use townjump_dht::{Dht, DhtFactory, Incoming, KeyPair, ServerOpts};

impl HyperTransport {
    /// Starts the node and the server; `listen` runs as a task (wait for its outcome with `wait_listen_outcome`).
    pub async fn bind(secret: NodeSecret, opts: HyperOpts) -> NetResult<Arc<Self>> {
        let kp = key_pair(&secret);
        let factory = factory_for(opts.bootstrap, opts.table_file.as_deref());
        let relay_factory = factory.clone();
        let node = Arc::new(
            factory
                .create(Some(kp.clone()))
                .await
                .map_err(dht_error_to_net_error)?,
        );
        let saver = opts
            .table_file
            .map(|path| TableSaver::start(node.clone(), path));
        let gate = Arc::new(PeerGate::open());
        // reusable_socket: same as townjump-server, so that clients follow the remembered route.
        let server_opts = ServerOpts {
            firewall: None,
            reusable_socket: true,
            ..Default::default()
        };
        let (server, incoming) = node
            .create_server(server_opts)
            .map_err(dht_error_to_net_error)?;
        let (accepted_tx, out) = mpsc::channel(ACCEPT_QUEUE);
        let dialer = Arc::new(NodeDialer {
            node: node.clone(),
            slots: Arc::new(Semaphore::new(DIAL_SLOTS)),
            hints: HintBook::default(),
            relay: RelayDial::default(),
        });
        let ctx = Arc::new(AcceptCtx::new(dialer.clone(), accepted_tx, gate.clone()));
        let this = Arc::new(Self {
            me: secret.peer_id(),
            node,
            server,
            gate,
            ctx,
            dialer,
            incoming: tokio::sync::Mutex::new(out),
            listen_outcome: watch::channel(None).0,
            closed: watch::channel(false).0,
            close_started: AtomicBool::new(false),
            saver,
            relay_host: std::sync::Mutex::new(None),
            relay_factory,
        });
        this.start_transport_tasks(incoming, kp, opts.watch_net)
            .await;
        Ok(this)
    }

    /// Starts the accept loop, the server announcement and, if asked, the network watcher.
    async fn start_transport_tasks(
        self: &Arc<Self>,
        incoming: Incoming,
        kp: KeyPair,
        watch_net: bool,
    ) {
        crate::node::output::spawn(accept_loop(incoming, self.ctx.clone()));
        crate::node::output::spawn(run_listen(
            self.listen_outcome.clone(),
            self.server.clone(),
            kp,
        ));
        if watch_net {
            self.watch_network().await;
        }
    }
}
