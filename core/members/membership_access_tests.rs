//! Tests of `Members` with a membership list behind it (`membership.rs`): `is_member` and `try_begin_dial` follow
//! the verdicts, `cut` closes a refused neighbour's connections, `hold_conn_until_closed` drops a connection that
//! arrived after the verdict. Child module of `membership_tests.rs`: `member_context`, `mem_pair`, `tracked` and the
//! other helpers are there.

use super::*;
use crate::members::access::tally::{Exclusion, How, Ledger};
use crate::members::access::view::{AccessView, Deny, Verdicts, ACCESS_CLOSE_CODE};
use crate::transport::net::contract::{pid, SEED_C};
use crate::transport::net::mem::conn::MemConn;
use bytes::Bytes;
use std::collections::{BTreeMap, BTreeSet};

/// How long a task that must stay blocked is given to (wrongly) finish.
const SETTLE: Duration = Duration::from_millis(100);

mod refusal_log {
    use super::*;
    use crate::members::access::view::Side;

    #[test]
    fn first_verified_conn_clears_the_refusal_of_that_peer() {
        // given: a refusal reported by node A, noted on the view of the members
        let (me, a) = (pid(1), pid(2));
        let members = Members::with_access(view_with(me, &[a], &[]));
        members.access().note_refusal(a, Side::Them, Deny::Excluded);
        // when: A completes a handshake and its first connection is counted
        assert!(members.add_verified_conn(a));
        // then: the old refusal is gone
        assert!(members.access().refusals().is_empty());
    }

    #[test]
    fn second_conn_of_a_member_changes_nothing() {
        // given: a member with one connection
        let (me, a) = (pid(1), pid(2));
        let members = Members::with_access(view_with(me, &[a], &[]));
        members.add_verified_conn(a);
        // when: a refusal is noted afterwards and a second connection is counted
        members.access().note_refusal(a, Side::Them, Deny::Excluded);
        assert!(!members.add_verified_conn(a));
        // then: the refusal stays
        assert_eq!(members.access().refusals().len(), 1);
    }
}

mod is_member {
    use super::*;

    #[test]
    fn is_member_false_when_access_denies() {
        // given: A and B hold verified connections; the list excludes A and lists B
        let (me, a, b) = (pid(1), pid(2), pid(3));
        let members = Members::with_access(view_with(me, &[b], &[a]));
        members.add_verified_conn(a);
        members.add_verified_conn(b);
        // when / then: only the listed neighbour is a member
        assert!(
            !members.is_member(a),
            "an excluded neighbour must not be a member"
        );
        assert!(members.is_member(b), "a listed neighbour must be a member");
    }

    #[test]
    fn is_member_follows_the_view_published_later() {
        // given: a pending view and a connected neighbour A
        let (me, a) = (pid(1), pid(2));
        let view = AccessView::pending();
        let members = Members::with_access(view.clone());
        members.add_verified_conn(a);
        assert!(
            members.is_member(a),
            "a pending view must not hide connected neighbours"
        );
        // when: the list excludes A
        view.publish(verdicts_with(me, &[], &[a]));
        // then: A stops being a member
        assert!(!members.is_member(a), "the verdict must apply at once");
        // when: the list takes A back
        view.publish(verdicts_with(me, &[a], &[]));
        // then: A is a member again
        assert!(
            members.is_member(a),
            "a relisted neighbour must be a member again"
        );
    }

    #[test]
    fn access_hands_out_the_view_the_members_use() {
        // given: members built over a pending view
        let (me, a) = (pid(1), pid(2));
        let members = Members::with_access(AccessView::pending());
        members.add_verified_conn(a);
        // when: a verdict is published through the handle that `access()` returns
        members.access().publish(verdicts_with(me, &[], &[a]));
        // then: the members see it
        assert!(
            !members.is_member(a),
            "access() must return the very view in use"
        );
    }

    #[test]
    fn is_member_false_without_a_connection_even_if_listed() {
        // given: A is listed but has no verified connection
        let (me, a) = (pid(1), pid(2));
        let members = Members::with_access(view_with(me, &[a], &[]));
        // when / then
        assert!(!members.is_member(a));
    }
}

mod dial {
    use super::*;

    #[test]
    fn try_begin_dial_false_for_denied_when_ready() {
        // given: a ready view: A is excluded, B is listed, X is unknown
        let (me, a, b, x) = (pid(1), pid(2), pid(3), pid(4));
        let members = Members::with_access(view_with(me, &[b], &[a]));
        // when / then: no dial to the excluded or the unlisted, a dial to the listed
        assert!(
            !members.try_begin_dial(a),
            "dial to an excluded node must be refused"
        );
        assert!(
            !members.try_begin_dial(x),
            "dial to an unlisted node must be refused"
        );
        assert!(
            members.try_begin_dial(b),
            "dial to a listed node must start"
        );
    }

    #[test]
    fn try_begin_dial_true_while_pending() {
        // given: a view that has published nothing yet
        let a = pid(2);
        let members = Members::with_access(AccessView::pending());
        // when / then: the dial starts
        assert!(
            members.try_begin_dial(a),
            "a pending view must not block dials"
        );
    }

    #[test]
    fn try_begin_dial_true_again_after_the_peer_is_relisted() {
        // given: A was refused while excluded
        let (me, a) = (pid(1), pid(2));
        let view = view_with(me, &[], &[a]);
        let members = Members::with_access(view.clone());
        assert!(!members.try_begin_dial(a));
        // when: the list takes A back
        view.publish(verdicts_with(me, &[a], &[]));
        // then: the refused attempt left no trace, the dial starts
        assert!(
            members.try_begin_dial(a),
            "a refused attempt must not stay in the dialing set"
        );
    }
}

mod cut {
    use super::*;

    #[tokio::test]
    async fn cut_closes_all_tracked_conns_of_peer() {
        // given: three tracked connections: two to B and one to C
        let dialed = dial_nodes(&[SEED_B, SEED_B, SEED_C]).await;
        let members = Members::default();
        dialed
            .conns
            .iter()
            .for_each(|conn| members.track_conn(conn));
        // when: B is cut
        let closed = members.cut(pid(SEED_B), Deny::Excluded);
        // then: both connections of B are closed with code 403 and the reason of the denial
        assert_eq!(closed, 2, "number of closed connections");
        let want = Some((ACCESS_CLOSE_CODE, Bytes::from_static(b"excluded")));
        assert_eq!(close_info(&dialed.conns[0]), want, "first connection of B");
        assert_eq!(close_info(&dialed.conns[1]), want, "second connection of B");
    }

    #[tokio::test]
    async fn cut_leaves_conns_of_other_peers_open() {
        // given: the same three connections
        let dialed = dial_nodes(&[SEED_B, SEED_B, SEED_C]).await;
        let members = Members::default();
        dialed
            .conns
            .iter()
            .for_each(|conn| members.track_conn(conn));
        // when: B is cut
        members.cut(pid(SEED_B), Deny::Excluded);
        // then: the connection to C is untouched
        assert!(
            !dialed.conns[2].is_closed(),
            "the connection to another peer was closed"
        );
    }

    #[tokio::test]
    async fn cut_of_a_peer_without_conns_closes_nothing() {
        // given: one tracked connection, to B
        let dialed = dial_nodes(&[SEED_B]).await;
        let members = Members::default();
        members.track_conn(&dialed.conns[0]);
        // when: C, which has no connection, is cut
        let closed = members.cut(pid(SEED_C), Deny::Unlisted);
        // then: nothing is closed
        assert_eq!(closed, 0);
        assert!(
            !dialed.conns[0].is_closed(),
            "the connection to B was closed"
        );
    }

    #[tokio::test]
    async fn cut_gives_the_wire_reason_of_the_denial() {
        // given: a tracked connection to B
        let dialed = dial_nodes(&[SEED_B]).await;
        let members = Members::default();
        members.track_conn(&dialed.conns[0]);
        // when: B is cut as unlisted
        members.cut(pid(SEED_B), Deny::Unlisted);
        // then: the close frame carries the text older nodes also see
        let want = Some((403, Bytes::from_static(b"not admitted")));
        assert_eq!(close_info(&dialed.conns[0]), want);
    }
}

mod hold_conn {
    use super::*;

    #[tokio::test]
    async fn hold_conn_closes_conn_of_denied_peer() {
        // given: B's side holds a connection of A, whom the list excludes
        let mem = mem_pair().await;
        let (b, a) = (mem.near.peer(), mem.far.peer());
        let (ctx, mut events) = member_context_with(b, view_with(b, &[], &[a]));
        ctx.members.add_verified_conn(a);
        // when: holding starts
        let holding = tokio::spawn(hold_conn_until_closed(mem.far.clone(), a, ctx.clone()));
        // then: the connection is closed with 403 on both ends, the holder ends and reports Down
        must_finish(holding).await.expect("hold");
        let want = Some((ACCESS_CLOSE_CODE, Bytes::from_static(b"excluded")));
        assert_eq!(close_info(&mem.far), want, "holding end");
        must_finish(mem.near.closed()).await;
        assert_eq!(close_info(&mem.near), want, "remote end");
        down_reason(&mut events).await;
        assert!(
            ctx.members.conns_snapshot().is_empty(),
            "the closed connection stayed registered"
        );
    }

    #[tokio::test]
    async fn hold_conn_keeps_conn_of_listed_peer() {
        // given: B's side holds a connection of A, whom the list admits
        let mem = mem_pair().await;
        let (b, a) = (mem.near.peer(), mem.far.peer());
        let (ctx, _events) = member_context_with(b, view_with(b, &[a], &[]));
        ctx.members.add_verified_conn(a);
        // when: holding starts
        let holding = tokio::spawn(hold_conn_until_closed(mem.far.clone(), a, ctx.clone()));
        tracked(&ctx.members, 1).await;
        tokio::time::sleep(SETTLE).await;
        // then: the connection stays open and held
        assert!(
            !mem.far.is_closed(),
            "a listed peer's connection was closed"
        );
        assert!(
            !holding.is_finished(),
            "the holder ended though nothing closed"
        );
        mem.near.close(0, b"done");
        must_finish(holding).await.expect("hold");
    }
}

// ---- helpers ----

/// Member context whose `Members` answer to `view`.
fn member_context_with(
    me: PeerId,
    view: AccessView,
) -> (MemberContext, mpsc::UnboundedReceiver<MemberEvent>) {
    let (mut ctx, events) = member_context(me);
    ctx.members = Members::with_access(view);
    (ctx, events)
}

/// A view that already holds the verdicts of `verdicts_with`.
fn view_with(me: PeerId, listed: &[PeerId], excluded: &[PeerId]) -> AccessView {
    let view = AccessView::open();
    view.publish(verdicts_with(me, listed, excluded));
    view
}

/// Verdicts of `me`: `listed` are admitted, `excluded` are admitted and then excluded by votes; no votes, no blocks.
fn verdicts_with(me: PeerId, listed: &[PeerId], excluded: &[PeerId]) -> Verdicts {
    let by_votes = || Exclusion {
        how: How::Votes,
        at_micros: 10,
        voters: Vec::new(),
    };
    let ledger = Ledger {
        admitted: [&[me][..], listed, excluded].concat().into_iter().collect(),
        excluded: excluded.iter().map(|p| (*p, by_votes())).collect(),
        pending: BTreeMap::new(),
        need: 1,
        admissions: BTreeMap::new(),
    };
    Verdicts {
        me,
        ledger: Arc::new(ledger),
        my_votes: BTreeSet::new(),
        blocks: BTreeSet::new(),
        reasons: Default::default(),
    }
}

/// Connections from node A to the nodes with the given seeds (a seed may repeat: several connections to one node).
/// The accepted ends and the nodes live as long as the struct.
struct Dialed {
    conns: Vec<Arc<dyn PeerConn>>,
    _accepted: Vec<Arc<dyn PeerConn>>,
    _nodes: Vec<Arc<SplitTransport>>,
}

async fn dial_nodes(seeds: &[u8]) -> Dialed {
    let net = MemNet::new();
    let a = net.transport(&secret(SEED_A));
    let mut nodes: BTreeMap<u8, Arc<SplitTransport>> = BTreeMap::new();
    for seed in seeds {
        nodes
            .entry(*seed)
            .or_insert_with(|| served_node(&net, *seed));
    }
    let (mut conns, mut accepted) = (Vec::new(), Vec::new());
    for seed in seeds {
        let b = &nodes[seed];
        let (near, far) = dial_once(&a, b, &b.local_addr()).await;
        conns.push(near);
        accepted.push(far);
    }
    let mut all: Vec<Arc<SplitTransport>> = nodes.into_values().collect();
    all.push(a);
    Dialed {
        conns,
        _accepted: accepted,
        _nodes: all,
    }
}

fn served_node(net: &MemNet, seed: u8) -> Arc<SplitTransport> {
    let node = net.transport(&secret(seed));
    node.serve(&[AUTH]);
    node
}

/// Code and reason of closing a `mem` connection; `None` — not closed.
fn close_info(conn: &Arc<dyn PeerConn>) -> Option<(u32, Bytes)> {
    let mem = conn.as_any().downcast_ref::<MemConn>();
    mem.expect("connection must be mem").close_info()
}
