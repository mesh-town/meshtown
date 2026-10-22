//! Tests of the facade, continued: the stack is dropped without `close()` — tasks are aborted, the transport is released.

use super::*;
use std::sync::Weak;

/// Polls the condition every 5 ms; time limit — `STEP`. Real time: cancellation happens on the runtime's poll.
async fn until(what: &str, cond: impl Fn() -> bool) {
    within(what, async {
        while !cond() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

/// Node B with a live connection in a handler and an eternal discovery source, everything writes to the log.
async fn busy_b(net: &Arc<MemNet>, script: &Arc<Script>, journal: &Journal) -> (Node, Node) {
    let source: Arc<dyn Discovery> = script.clone();
    let opts = Opts {
        probe: Probe::journaled(journal),
        sources: vec![source],
        journal: Some(journal.clone()),
        ..serving(&[AUTH])
    };
    let (a, b) = (node(net, 1, Opts::default()), node(net, 2, opts));
    a.stack.add_hints(b.invite());
    let live = a.connect(2, AUTH).await.expect("connect succeeded");
    drop(b.probe.next().await);
    drop(live);
    (a, b)
}

#[tokio::test]
async fn o2_dropped_stack_aborts_tasks_and_releases_transport() {
    // given: B with a live connection in a handler and eternal discovery
    let net = MemNet::new();
    let journal = Journal::new();
    let script = Script::new("mainline").journaled(&journal).shared();
    let (_a, b) = busy_b(&net, &script, &journal).await;
    let spy: Weak<Spy> = Arc::downgrade(&b.spies[0]);

    // when: B is dropped entirely, without close()
    drop(b);

    // then: tasks are aborted, the transport is released, but not closed via close()
    until("transport released", || spy.strong_count() == 0).await;
    until("discovery and handler aborted", || {
        let seen = journal.entries();
        seen.contains(&"discovery") && seen.contains(&"handler")
    })
    .await;
    assert!(
        !journal.entries().contains(&"transport"),
        "close() was not called, the transport does not write to the log"
    );
}

#[tokio::test]
async fn o2_close_then_drop_is_quiet() {
    // given: B is closed with close()
    let net = MemNet::new();
    let journal = Journal::new();
    let script = Script::new("mainline").journaled(&journal).shared();
    let (_a, b) = busy_b(&net, &script, &journal).await;
    let spy: Weak<Spy> = Arc::downgrade(&b.spies[0]);
    within("close", b.stack.close()).await;

    // when: a second close(), then drop
    within("second close", b.stack.close()).await;
    let closes = b.spies[0].closes();
    drop(b);

    // then: the transport is closed once, the log has nothing extra, everything is released
    assert_eq!(closes, 1, "transport close — exactly one");
    assert_eq!(journal.entries(), vec!["discovery", "handler", "transport"]);
    until("transport released", || spy.strong_count() == 0).await;
}

#[tokio::test]
async fn o2_dropping_a_clone_keeps_the_stack() {
    // given: B and a clone of its stack
    let net = MemNet::new();
    let script = Script::new("mainline").shared();
    let source: Arc<dyn Discovery> = script.clone();
    let opts = Opts {
        sources: vec![source],
        ..serving(&[AUTH])
    };
    let (a, b) = (node(&net, 1, Opts::default()), node(&net, 2, opts));
    a.stack.add_hints(b.invite());
    let clone = b.stack.clone();

    // when: the clone is dropped
    drop(clone);

    // then: B accepts connections, discovery is alive, close() closes the transport once
    let _conn = a.connect(2, AUTH).await.expect("connect succeeded");
    drop(b.probe.next().await);
    sleep(Duration::from_millis(50)).await;
    assert!(
        !script.dropped(),
        "discovery is alive while the stack is alive"
    );
    within("close", b.stack.close()).await;
    assert_eq!(b.spies[0].closes(), 1);
}
