//! The relay node of this member: started once its owner has a public address, announced to the network
//! while that holds, and stopped with the transport.

use crate::transport::net::stream::BoxFuture;
use crate::transport::net::task_label;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use townjump_dht::{DhtError, NodeMode};
use townjump_dht_peeroxide::PeeroxideRelay;

/// How often the node mode is polled.
pub(crate) const RELAY_MODE_POLL: Duration = Duration::from_secs(30);
/// How long `close` waits for the host task: the relay's own 5 s close plus a margin.
const HOST_CLOSE_WAIT: Duration = Duration::from_secs(6);

/// Starts the relay node.
pub(crate) type StartRelay =
    Arc<dyn Fn() -> BoxFuture<'static, Result<PeeroxideRelay, DhtError>> + Send + Sync>;

/// A relay is served by public (not firewalled) persistent nodes only.
pub(crate) fn relay_wanted(mode: &NodeMode) -> bool {
    mode.persistent && !mode.firewalled
}

/// What the host does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostStep {
    Start,
    Advertise([u8; 32]),
    Retract,
    Stay,
}

/// What the host has done so far: the relay starts once per life, the announcement comes and goes.
#[derive(Debug, Default)]
pub(crate) struct HostState {
    key: Option<[u8; 32]>,
    advertised: bool,
}

impl HostState {
    pub(crate) fn step(&self, wanted: bool) -> HostStep {
        match (self.key, wanted, self.advertised) {
            (None, true, _) => HostStep::Start,
            (Some(key), true, false) => HostStep::Advertise(key),
            (Some(_), false, true) => HostStep::Retract,
            _ => HostStep::Stay,
        }
    }

    pub(crate) fn started(&mut self, key: [u8; 32]) {
        self.key = Some(key);
    }

    pub(crate) fn advertised(&mut self, on: bool) {
        self.advertised = on;
    }
}

/// The running host: the announced key and the stop switch.
pub(crate) struct RelayHost {
    keys: watch::Receiver<Option<[u8; 32]>>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl RelayHost {
    /// The announced relay key: `Some` while announced, `None` otherwise.
    pub(crate) fn keys(&self) -> watch::Receiver<Option<[u8; 32]>> {
        self.keys.clone()
    }

    /// Stops the host and its relay; gives up on the task after `HOST_CLOSE_WAIT`.
    pub(crate) async fn close(self) {
        let _ = self.stop.send(true);
        let mut task = self.task;
        if tokio::time::timeout(HOST_CLOSE_WAIT, &mut task)
            .await
            .is_err()
        {
            task.abort();
        }
    }
}

/// Starts the host task: a step right away and then once per `poll`.
pub(crate) fn spawn_host(
    mode: Arc<dyn Fn() -> NodeMode + Send + Sync>,
    start: StartRelay,
    poll: Duration,
) -> RelayHost {
    let (keys_tx, keys) = watch::channel(None);
    let (stop, stop_rx) = watch::channel(false);
    let host = Host {
        mode,
        start,
        keys: keys_tx,
        stop: stop_rx,
    };
    let task = task_label::spawn(host.run(poll));
    RelayHost { keys, stop, task }
}

struct Host {
    mode: Arc<dyn Fn() -> NodeMode + Send + Sync>,
    start: StartRelay,
    keys: watch::Sender<Option<[u8; 32]>>,
    stop: watch::Receiver<bool>,
}

impl Host {
    async fn run(mut self, poll: Duration) {
        let mut state = HostState::default();
        let mut relay = None;
        loop {
            self.tick(&mut state, &mut relay).await;
            if self.pause(poll).await {
                break;
            }
        }
        // Only a change is signalled: a key channel that holds `None` already ends with the sender.
        self.keys.send_if_modified(|key| key.take().is_some());
        close_relay(relay).await;
    }

    /// Waits one poll interval; `true` when the host was stopped (or dropped) meanwhile.
    async fn pause(&mut self, poll: Duration) -> bool {
        let stop = self.stop.wait_for(|s| *s);
        tokio::time::timeout(poll, stop).await.is_ok()
    }

    async fn tick(&mut self, state: &mut HostState, relay: &mut Option<PeeroxideRelay>) {
        let wanted = relay_wanted(&(self.mode)());
        let step = match state.step(wanted) {
            HostStep::Start => self.start_relay(state, relay).await,
            other => other,
        };
        self.apply(state, step);
    }

    /// Starts the relay unless stopped meanwhile; the next step follows the outcome.
    async fn start_relay(
        &mut self,
        state: &mut HostState,
        relay: &mut Option<PeeroxideRelay>,
    ) -> HostStep {
        let outcome = tokio::select! {
            _ = self.stop.wait_for(|s| *s) => return HostStep::Stay,
            outcome = (self.start)() => outcome,
        };
        let started = outcome.map_err(|e| tracing::warn!("relay: start failed: {e}"));
        let Ok(node) = started else {
            return HostStep::Stay;
        };
        let key = node.public_key();
        tracing::debug!("relay: serving this network as {}", short(&key));
        state.started(key);
        *relay = Some(node);
        HostStep::Advertise(key)
    }

    fn apply(&self, state: &mut HostState, step: HostStep) {
        match step {
            HostStep::Advertise(key) => self.announce(state, Some(key)),
            HostStep::Retract => self.announce(state, None),
            HostStep::Start | HostStep::Stay => {}
        }
    }

    fn announce(&self, state: &mut HostState, key: Option<[u8; 32]>) {
        state.advertised(key.is_some());
        self.keys.send_replace(key);
    }
}

async fn close_relay(relay: Option<PeeroxideRelay>) {
    if let Some(node) = relay {
        node.close().await;
    }
}

fn short(key: &[u8; 32]) -> String {
    key.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
