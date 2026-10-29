//! The relay node of this member: started while its owner looks public, announced to the network once the relay
//! has checked its own address, closed when no address shows up, and stopped with the transport.

use crate::transport::net::stream::BoxFuture;
use crate::transport::net::task_label;
use crate::transport::net_hyper::relay::RelayAdvert;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use townjump_dht::{DhtError, NodeMode};
use townjump_dht_peeroxide::PeeroxideRelay;

/// How often the node mode is polled.
pub(crate) const RELAY_MODE_POLL: Duration = Duration::from_secs(30);
/// How long `close` waits for the host task: the relay's own 5 s close plus a margin.
const HOST_CLOSE_WAIT: Duration = Duration::from_secs(6);

/// How long a running relay may stay without a checked public address before it is closed.
pub(crate) const REACH_GRACE: Duration = Duration::from_secs(90);
/// How long after closing an unreachable relay the next one may start.
pub(crate) const RELAY_RETRY: Duration = Duration::from_secs(600);

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
    Advertise(RelayAdvert),
    Retract,
    Restart,
    Stay,
}

/// What the host has done so far: the relay starts, is announced once its address is checked, and is closed
/// when no address shows up in time.
#[derive(Debug, Default)]
pub(crate) struct HostState {
    running: bool,
    advertised: Option<RelayAdvert>,
    /// When the wait for an address began: the start of the relay or the loss of the address.
    since: Option<Instant>,
    /// No new relay starts before this moment.
    retry_at: Option<Instant>,
}

impl HostState {
    pub(crate) fn step(&self, wanted: bool, seen: Option<RelayAdvert>, now: Instant) -> HostStep {
        match (self.running, wanted, self.advertised, seen) {
            (false, true, ..) => self.start_or_wait(now),
            (true, true, prev, Some(s)) if prev != Some(s) => HostStep::Advertise(s),
            (true, true, Some(_), None) | (true, false, Some(_), _) => HostStep::Retract,
            (true, true, None, None) if self.grace_over(now) => HostStep::Restart,
            _ => HostStep::Stay,
        }
    }

    fn start_or_wait(&self, now: Instant) -> HostStep {
        match self.retry_at {
            Some(at) if now < at => HostStep::Stay,
            _ => HostStep::Start,
        }
    }

    fn grace_over(&self, now: Instant) -> bool {
        self.since
            .is_some_and(|since| now.saturating_duration_since(since) >= REACH_GRACE)
    }

    pub(crate) fn started(&mut self, now: Instant) {
        self.running = true;
        self.since = Some(now);
    }

    pub(crate) fn advertised(&mut self, advert: Option<RelayAdvert>, now: Instant) {
        self.advertised = advert;
        self.since = Some(now);
    }

    pub(crate) fn stopped(&mut self, now: Instant) {
        *self = Self {
            retry_at: Some(now + RELAY_RETRY),
            ..Self::default()
        };
    }
}

/// The running host: the announced relay and the stop switch.
pub(crate) struct RelayHost {
    keys: watch::Receiver<Option<RelayAdvert>>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl RelayHost {
    /// The announced relay (key and address): `Some` while announced, `None` otherwise.
    pub(crate) fn keys(&self) -> watch::Receiver<Option<RelayAdvert>> {
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
    keys: watch::Sender<Option<RelayAdvert>>,
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
        let mut step = state.step(wanted, seen_address(relay).await, Instant::now());
        if step == HostStep::Start {
            self.start_relay(state, relay).await;
            step = state.step(wanted, seen_address(relay).await, Instant::now());
        }
        self.apply(state, relay, step).await;
    }

    /// Starts the relay unless stopped meanwhile; a failed start is retried at the next tick.
    async fn start_relay(&mut self, state: &mut HostState, relay: &mut Option<PeeroxideRelay>) {
        let outcome = tokio::select! {
            _ = self.stop.wait_for(|s| *s) => return,
            outcome = (self.start)() => outcome,
        };
        let started = outcome.map_err(|e| tracing::warn!("relay: start failed: {e}"));
        let Ok(node) = started else {
            return;
        };
        tracing::debug!("relay: started as {}", short(&node.public_key()));
        state.started(Instant::now());
        *relay = Some(node);
    }

    async fn apply(
        &self,
        state: &mut HostState,
        relay: &mut Option<PeeroxideRelay>,
        step: HostStep,
    ) {
        match step {
            HostStep::Advertise(advert) => self.announce(state, Some(advert)),
            HostStep::Retract => self.announce(state, None),
            HostStep::Restart => self.restart(state, relay).await,
            HostStep::Start | HostStep::Stay => {}
        }
    }

    fn announce(&self, state: &mut HostState, advert: Option<RelayAdvert>) {
        state.advertised(advert, Instant::now());
        self.keys.send_replace(advert);
    }

    /// Closes the relay that found no address; the next one starts after `RELAY_RETRY`.
    async fn restart(&self, state: &mut HostState, relay: &mut Option<PeeroxideRelay>) {
        tracing::warn!("relay: no public address confirmed, closing the relay");
        self.keys.send_replace(None);
        close_relay(relay.take()).await;
        state.stopped(Instant::now());
    }
}

/// The address the running relay has confirmed; none while unknown or when the relay cannot tell.
async fn seen_address(relay: &Option<PeeroxideRelay>) -> Option<RelayAdvert> {
    let node = relay.as_ref()?;
    let address = node.relay_address().await.ok().flatten();
    address.map(RelayAdvert::from)
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

#[cfg(test)]
#[path = "host_reach_tests.rs"]
mod reach_tests;
