//! The node's main loop (`run_node_loop`): handles discovered addresses, member events,
//! dial failures, retry deadlines and periodic tasks.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::transport::net::{AddrHint, Membership, NetStack, PeerAddr, PeerConn, PeerId};
use bytes::Bytes;
use tokio::sync::mpsc;
use tokio::time::{interval_at, Instant, Interval, MissedTickBehavior};

use crate::members::membership::{MemberContext, MemberEvent, Members};
use crate::members::peer_names::PeerNames;
use crate::members::stale_prune::{self, PruneSources};
use crate::node::output::{say, short_peer_id};
use crate::node::telemetry::{self, Telemetry};
use crate::proxy::domains::DomainList;
use crate::proxy::route::RouteCtx;
use crate::site_checks::checks::Checks;
use crate::transport::net_mainline;
use crate::transport::peers_cache::{self, PeersCache};

use super::dial::{DialFailed, PeerDialer};
use super::peer_path;
use super::rejoin::{rejoin_if_alone, Rejoin};

/// Invites a peer into the gossip topic and starts a domain list sync with it.
///
/// join — on every Up, from both sides: with counter-calls Up may arrive on both from
/// respond, and "only the initiator invites" would leave the topic empty.
async fn invite_and_sync(peer: PeerId, swarm: &dyn Membership, domains: &DomainList) {
    match swarm.join(&[peer]).await {
        Ok(()) => say(
            "gossip",
            format!("inviting {} to the topic", short_peer_id(&peer)),
        ),
        Err(e) => say(
            "gossip",
            format!("couldn't invite {}: {e}", short_peer_id(&peer)),
        ),
    }
    match domains.sync_shared_with(peer).await {
        Ok(()) => say(
            "docs",
            format!("syncing the network document with {}", short_peer_id(&peer)),
        ),
        Err(e) => say(
            "docs",
            format!(
                "couldn't start syncing with {}: {e:#}",
                short_peer_id(&peer)
            ),
        ),
    }
}

/// Collects and, if changed, writes to disk the address cache of a confirmed member
/// (`MemberEvent::Up`).
fn remember_peer_and_save(cache: &mut PeersCache, peer: PeerId, selected: Option<AddrHint>) {
    // The stack doesn't hand out known addresses: `hyper` has no hints, and the cache rejects an entry without addresses.
    let entry = peers_cache::entry_for(None, peer, selected.as_ref(), net_mainline::unix_now());
    if cache.remember(entry, net_mainline::unix_now()) {
        save_cache(cache);
    }
}

/// Writes the address cache to disk; an error is a log line, not a failure.
fn save_cache(cache: &PeersCache) {
    if let Err(e) = cache.save() {
        say("cache", format!("address cache not written: {e}"));
    }
}

/// Log line about a connection to a member coming up.
fn up_line(who: impl std::fmt::Display, count: usize) -> String {
    format!("+ {} (peers: {})", who, count)
}

/// Log line about a member vanishing.
fn down_line(who: impl std::fmt::Display, reason: &str, count: usize) -> String {
    format!("- {}: {reason} (peers: {})", who, count)
}

/// Broadcasts the periodic `hello` into the gossip topic if the network has at least one member.
async fn broadcast_hello(
    swarm: &dyn Membership,
    id: PeerId,
    members: &Members,
    hello_count: u64,
) -> u64 {
    if members.count() == 0 {
        return hello_count;
    }
    let hello_count = hello_count + 1;
    let text = format!("hello from {} #{hello_count}", short_peer_id(&id));
    if let Err(e) = swarm.broadcast(Bytes::from(text)).await {
        say("gossip", format!("send error: {e}"));
    }
    hello_count
}

/// Logs the node's periodic status: members, gossip neighbors, domains, checks, memory.
async fn log_periodic_status(
    members: &Members,
    gossip_neighbor_count: &Arc<AtomicUsize>,
    domains: &DomainList,
    checks: &Checks,
) {
    let rss = telemetry::rss_kib().0;
    let rss_text = if rss == 0 {
        "n/a".to_string()
    } else {
        rss.to_string()
    };
    let domains_text = domains
        .count()
        .await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| "n/a".to_string());
    let checks_text = checks
        .own_count()
        .await
        .map(|n| n.to_string())
        .unwrap_or_else(|_| "n/a".to_string());
    say(
        "status",
        format!(
            "peers {}, gossip neighbors {}, domains {domains_text}, checks {checks_text}, {}, memory {rss_text} KiB",
            members.count(),
            gossip_neighbor_count.load(Ordering::Relaxed),
            members.conns_text(),
        ),
    );
}

/// The loop's three periodic timers: broadcast, status line and cleanup of stale entries.
fn periodic_ticks(start: Instant) -> (Interval, Interval, Interval) {
    let broadcast_tick = interval_at(start + Duration::from_secs(30), Duration::from_secs(30));
    let status_tick = interval_at(
        Instant::now() + Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let mut prune_tick = interval_at(
        Instant::now() + stale_prune::FIRST_PRUNE_AFTER,
        stale_prune::PRUNE_EVERY,
    );
    // After sleeping for more than a day, missed ticks must not arrive in a batch: the "two ticks in a row" rule
    // (`stale_prune`) relies on the hour-long gap between them.
    prune_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    (broadcast_tick, status_tick, prune_tick)
}

/// Shared handles of the node loop: read on every wake-up, the loop doesn't change them.
pub(super) struct LoopCtx {
    pub(super) id: PeerId,
    pub(super) stack: NetStack,
    pub(super) members: Members,
    pub(super) swarm: Arc<dyn Membership>,
    pub(super) gossip_neighbor_count: Arc<AtomicUsize>,
    pub(super) domains: DomainList,
    pub(super) checks: Checks,
    pub(super) telemetry: Arc<Telemetry>,
    pub(super) route: RouteCtx,
}

/// What the loop takes for itself: the dial context, incoming channels, the address cache and names.
pub(super) struct LoopInputs {
    pub(super) ctx: MemberContext,
    pub(super) found: mpsc::Receiver<PeerAddr>,
    pub(super) events: mpsc::UnboundedReceiver<MemberEvent>,
    pub(super) cache: PeersCache,
    pub(super) names: PeerNames,
}

/// Mutable state of the loop.
struct LoopState {
    dialer: PeerDialer,
    cache: PeersCache,
    names: PeerNames,
    rejoin: Rejoin,
    prune_candidates_prev_tick: HashSet<PeerId>,
    hello_count: u64,
}

impl LoopState {
    /// Handles one wake-up; `false` — the loop ends.
    async fn dispatch_wake(&mut self, loop_ctx: &LoopCtx, wake: Wake) -> bool {
        match wake {
            Wake::Found(addr) => self.dialer.on_addr_found(addr),
            Wake::Member(event) => self.dispatch_member_event(loop_ctx, event).await,
            Wake::Failed(failed) => self.dialer.on_dial_failed(failed),
            Wake::RedialDue => self.dialer.dial_due_peers(),
            Wake::Allowed => self.dialer.on_access_allowed(),
            Wake::HelloTick => self.hello(loop_ctx).await,
            Wake::StatusTick => {
                log_periodic_status(
                    &loop_ctx.members,
                    &loop_ctx.gossip_neighbor_count,
                    &loop_ctx.domains,
                    &loop_ctx.checks,
                )
                .await
            }
            Wake::PruneTick => self.prune(loop_ctx).await,
            Wake::Idle => {}
            Wake::Closed => return false,
        }
        true
    }

    /// Broadcasts `hello` and, when alone, calls back into the swarm.
    async fn hello(&mut self, loop_ctx: &LoopCtx) {
        self.hello_count = broadcast_hello(
            &*loop_ctx.swarm,
            loop_ctx.id,
            &loop_ctx.members,
            self.hello_count,
        )
        .await;
        let gossip_neighbor_count = loop_ctx.gossip_neighbor_count.load(Ordering::Relaxed);
        rejoin_if_alone(
            &*loop_ctx.swarm,
            &mut self.rejoin,
            gossip_neighbor_count,
            &loop_ctx.members,
            Instant::now(),
        )
        .await;
    }

    /// Cleans up stale entries and makes the dialer forget members that left.
    async fn prune(&mut self, loop_ctx: &LoopCtx) {
        let src = PruneSources {
            id: loop_ctx.id,
            members: &loop_ctx.members,
            telemetry: &loop_ctx.telemetry,
            domains: &loop_ctx.domains,
            checks: &loop_ctx.checks,
            route: &loop_ctx.route,
        };
        let pruned = stale_prune::prune_tick(
            &src,
            &mut self.prune_candidates_prev_tick,
            &mut self.cache,
            &mut self.names,
            net_mainline::unix_now(),
        )
        .await;
        self.dialer.forget_pruned_peers(&pruned);
    }

    /// Handles a member event: a connection coming up (path, telemetry, invite into the gossip
    /// topic, domain list sync, write to the address cache and the names file, retry cancel) or its
    /// disappearance (cache refresh, retry scheduling).
    async fn dispatch_member_event(&mut self, loop_ctx: &LoopCtx, event: MemberEvent) {
        match event {
            MemberEvent::Up { peer, conn } => self.on_up(loop_ctx, peer, conn).await,
            MemberEvent::Down { peer, reason } => self.on_down(loop_ctx, peer, &reason),
        }
    }

    async fn on_up(&mut self, loop_ctx: &LoopCtx, peer: PeerId, conn: Arc<dyn PeerConn>) {
        say(
            "member",
            up_line(loop_ctx.members.peer_label(peer), loop_ctx.members.count()),
        );
        self.dialer.on_peer_up(peer);
        let selected = peer_path::record_peer_up(&*conn, &loop_ctx.stack, &loop_ctx.telemetry);
        remember_peer_and_save(&mut self.cache, peer, selected);
        self.names.remember_member_and_save(&loop_ctx.members, peer);
        crate::node::output::spawn(peer_path::watch_path(
            conn.path_watch(),
            peer,
            loop_ctx.telemetry.clone(),
        ));
        invite_and_sync(peer, &*loop_ctx.swarm, &loop_ctx.domains).await;
    }

    fn on_down(&mut self, loop_ctx: &LoopCtx, peer: PeerId, reason: &str) {
        say(
            "member",
            down_line(
                loop_ctx.members.peer_label(peer),
                reason,
                loop_ctx.members.count(),
            ),
        );
        self.dialer.on_peer_down(peer, reason);
        loop_ctx.telemetry.peer_down(peer);
        if self.cache.touch(peer, net_mainline::unix_now()) {
            save_cache(&self.cache);
        }
    }
}

/// The node's main loop: handles discovered addresses, member events, dial failures, retry
/// deadlines and periodic tasks.
pub(super) async fn run_node_loop(loop_ctx: LoopCtx, inputs: LoopInputs) {
    let start = Instant::now();
    let rejoin = Rejoin::new(start);
    let (broadcast, status, prune) = periodic_ticks(start);
    let prune_candidates_prev_tick = HashSet::new();
    let hello_count = 0u64;
    let (failed_tx, failed) = mpsc::unbounded_channel::<DialFailed>();
    let allowed_rx = inputs.ctx.members.access().allowed_changes();
    let dialer = PeerDialer::new(loop_ctx.id, loop_ctx.stack.clone(), inputs.ctx, failed_tx);
    let mut state = LoopState {
        dialer,
        cache: inputs.cache,
        names: inputs.names,
        rejoin,
        prune_candidates_prev_tick,
        hello_count,
    };
    let mut wakers = Wakers {
        found_rx: inputs.found,
        events_rx: inputs.events,
        failed_rx: failed,
        allowed_rx,
        hello_interval: broadcast,
        status_interval: status,
        prune_interval: prune,
    };
    loop {
        let wake = wakers.next(state.dialer.next_redial_at()).await;
        if !state.dispatch_wake(&loop_ctx, wake).await {
            break;
        }
    }
}

#[path = "event_loop_wake.rs"]
mod wake;
use wake::{Wake, Wakers};

#[cfg(test)]
#[path = "event_loop_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "hello_golden_tests.rs"]
mod hello_golden_tests;
