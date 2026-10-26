//! Tests of the verdict view (`view.rs`): open and pending views, the order of denial reasons, the refusal of
//! the unlisted, what a publication reports as newly denied, the refusal log and the close reasons on the wire.
//! Nodes: `m` is the own node, `a`..`d` are listed or unlisted neighbours, `x` is a stranger.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::members::access::tally::{Exclusion, How, Ledger};
use crate::members::access::view::{AccessView, Deny, Side, Verdicts, ACCESS_CLOSE_CODE};
use crate::transport::net::contract::{must_finish, pid};
use crate::transport::net::PeerId;

/// How long a task that must stay blocked is given to (wrongly) finish.
const SETTLE: Duration = Duration::from_millis(100);

mod open_view {
    use super::*;

    #[tokio::test]
    async fn open_view_allows_everyone() {
        // given: an open view and a handful of nodes nobody has heard of
        let Cast { m, a, b, x, .. } = cast();
        let view = AccessView::open();
        // when: each is checked
        let got = outcomes(&view, &[m, a, b, x]);
        // then: all pass, `ready` returns at once, no verdicts were ever published
        assert_eq!(got, vec![Ok(()); 4]);
        assert!(
            [m, a, b, x].iter().all(|p| view.allows(*p)),
            "allows() must agree with check()"
        );
        must_finish(view.ready()).await;
        assert!(view.verdicts().is_none(), "an open view holds no verdicts");
    }

    #[tokio::test]
    async fn default_view_is_open() {
        // given: a view made by `Default`
        let Cast { x, .. } = cast();
        let view = AccessView::default();
        // when / then: it behaves as `open`
        assert_eq!(view.check(x), Ok(()));
        must_finish(view.ready()).await;
    }
}

mod pending_view {
    use super::*;

    #[tokio::test]
    async fn pending_view_allows_and_ready_waits() {
        // given: a pending view and a task waiting for it
        let Cast { m, a, x, .. } = cast();
        let view = AccessView::pending();
        let waiter = crate::node::output::spawn({
            let view = view.clone();
            async move { view.ready().await }
        });
        tokio::time::sleep(SETTLE).await;
        // then: until the first publication everybody is allowed, and `ready` has not returned
        assert!(view.allows(x), "a pending view must allow everyone");
        assert_eq!(view.check(x), Ok(()));
        assert!(
            !waiter.is_finished(),
            "ready returned before the first publication"
        );
        // when: the first publication arrives
        view.publish(verdicts(m, ledger(&[m, a], &[])));
        // then: the waiter is released
        must_finish(waiter).await.expect("waiter task");
    }

    #[tokio::test]
    async fn ready_returns_at_once_after_the_first_publication() {
        // given: a pending view that has been published already
        let Cast { m, a, .. } = cast();
        let view = AccessView::pending();
        view.publish(verdicts(m, ledger(&[m, a], &[])));
        // when / then: a late `ready` does not wait
        must_finish(view.ready()).await;
    }

    #[test]
    fn first_publication_replaces_the_allow_all_answer() {
        // given: a pending view
        let Cast { m, a, b, .. } = cast();
        let view = AccessView::pending();
        // when: verdicts exclude B
        view.publish(verdicts(m, ledger(&[m, a], &[b])));
        // then: the view now follows them and exposes them
        assert_eq!(view.check(b), Err(Deny::Excluded));
        assert_eq!(view.check(a), Ok(()));
        assert!(
            view.verdicts().is_some(),
            "published verdicts must be readable"
        );
    }
}

mod check_order {
    use super::*;

    #[test]
    fn check_order_left_excluded_blocked_unlisted() {
        // given: B is excluded and blocked; C is blocked; D is blocked and
        // not listed; X is not listed; A is plain
        let Cast { m, a, b, c, d, x } = cast();
        let view = view_of(Verdicts {
            blocks: set(&[b, c, d]),
            ..verdicts(m, ledger(&[m, a, c], &[b]))
        });
        // when: each is checked
        let got = outcomes(&view, &[a, b, c, d, x]);
        // then: the first reason in the fixed order wins
        let want = vec![
            Ok(()),
            Err(Deny::Excluded),
            Err(Deny::Blocked),
            Err(Deny::Blocked),
            Err(Deny::Unlisted),
        ];
        assert_eq!(got, want, "results for A, B, C, D, X");
        // given: now I left myself, B is excluded too
        let view = view_of(Verdicts {
            ..verdicts(m, ledger_left(&[a, c], &[b], m))
        });
        // when / then: "left" comes before everything, whoever is asked
        let got = outcomes(&view, &[a, b, c, x]);
        assert_eq!(got, vec![Err(Deny::Left); 4], "results for A, B, C, X");
    }

    #[test]
    fn self_is_allowed_whatever_the_verdicts_say() {
        // given: I am excluded, and also blocked by my own record
        let Cast { m, a, .. } = cast();
        let view = view_of(Verdicts {
            blocks: set(&[m]),
            ..verdicts(m, ledger(&[a], &[m]))
        });
        // when / then: asking about myself passes
        assert_eq!(view.check(m), Ok(()));
        assert!(view.allows(m));
    }

    #[test]
    fn listed_member_passes() {
        // given: A is listed, nothing against it
        let Cast { m, a, .. } = cast();
        let view = view_of(verdicts(m, ledger(&[m, a], &[])));
        // when / then
        assert_eq!(view.check(a), Ok(()));
        assert!(view.allows(a));
    }

    #[test]
    fn allows_is_false_for_a_denied_node() {
        // given: B is excluded
        let Cast { m, a, b, .. } = cast();
        let view = view_of(verdicts(m, ledger(&[m, a], &[b])));
        // when / then
        assert!(!view.allows(b), "an excluded node must not be allowed");
        assert!(view.allows(a));
    }
}

mod unlisted {
    use super::*;

    #[test]
    fn unlisted_is_refused_even_right_after_boot() {
        // given: X is not on the list and nothing else is going on
        let Cast { m, a, x, .. } = cast();
        let view = view_of(verdicts(m, ledger(&[m, a], &[])));
        // when / then: X is refused at once
        assert_eq!(view.check(x), Err(Deny::Unlisted));
        assert_eq!(view.check(a), Ok(()));
    }

    #[test]
    fn excluded_or_blocked_keep_their_own_reason() {
        // given: B is excluded, D is blocked by me
        let Cast { m, a, b, d, .. } = cast();
        let view = view_of(Verdicts {
            blocks: set(&[d]),
            ..verdicts(m, ledger(&[m, a], &[b]))
        });
        // when / then
        let got = outcomes(&view, &[b, d]);
        assert_eq!(got, vec![Err(Deny::Excluded), Err(Deny::Blocked)]);
    }

    #[test]
    fn everybody_gets_left_when_i_am_excluded() {
        // given: I left the network myself
        let Cast { m, a, x, .. } = cast();
        let view = view_of(verdicts(m, ledger_left(&[a], &[], m)));
        // when / then
        assert_eq!(outcomes(&view, &[a, x]), vec![Err(Deny::Left); 2]);
    }
}

#[path = "view_publish_tests.rs"]
mod publishing;

#[path = "view_allowed_tests.rs"]
mod allowed;

// ---- helpers ----

struct Cast {
    m: PeerId,
    a: PeerId,
    b: PeerId,
    c: PeerId,
    d: PeerId,
    x: PeerId,
}

fn cast() -> Cast {
    Cast {
        m: pid(1),
        a: pid(2),
        b: pid(3),
        c: pid(4),
        d: pid(5),
        x: pid(6),
    }
}

fn set(ids: &[PeerId]) -> BTreeSet<PeerId> {
    ids.iter().copied().collect()
}

fn map(rows: &[(PeerId, Deny)]) -> BTreeMap<PeerId, Deny> {
    rows.iter().copied().collect()
}

fn denied(rows: Vec<(PeerId, Deny)>) -> BTreeMap<PeerId, Deny> {
    rows.into_iter().collect()
}

/// A ledger where `listed` are admitted and `out` are admitted and excluded by votes.
fn ledger(listed: &[PeerId], out: &[PeerId]) -> Ledger {
    let by_votes = || Exclusion {
        how: How::Votes,
        at_micros: 10,
        voters: Vec::new(),
    };
    Ledger {
        admitted: listed.iter().chain(out).copied().collect(),
        excluded: out.iter().map(|p| (*p, by_votes())).collect(),
        admissions: BTreeMap::new(),
        admins: Default::default(),
        closed: None,
    }
}

/// Like `ledger`, and `left` also left the network on its own.
fn ledger_left(listed: &[PeerId], out: &[PeerId], left: PeerId) -> Ledger {
    let mut l = ledger(listed, out);
    l.admitted.insert(left);
    let how = Exclusion {
        how: How::Left,
        at_micros: 10,
        voters: vec![left],
    };
    l.excluded.insert(left, how);
    l
}

/// Verdicts of `me` over `ledger` with no votes, no blocks.
fn verdicts(me: PeerId, ledger: Ledger) -> Verdicts {
    Verdicts {
        me,
        ledger: Arc::new(ledger),
        blocks: BTreeSet::new(),
    }
}

/// An open view that already holds `v`.
fn view_of(v: Verdicts) -> AccessView {
    let view = AccessView::open();
    view.publish(v);
    view
}

fn outcomes(view: &AccessView, peers: &[PeerId]) -> Vec<Result<(), Deny>> {
    peers.iter().map(|p| view.check(*p)).collect()
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before 1970")
        .as_secs()
}

/// The close text a neighbour sees for `deny`, as `PeerConn::close_reason` reports it.
fn on_the_wire(deny: Deny) -> String {
    let reason = String::from_utf8(deny.wire_reason().to_vec()).expect("wire reason is text");
    format!("code {ACCESS_CLOSE_CODE}: {reason}")
}
