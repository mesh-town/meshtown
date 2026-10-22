//! Helpers of the replica cases (part of `replica_cases.rs`): opening, writing, reading, waiting for events.

use super::*;

pub(crate) fn val(s: &str) -> Bytes {
    Bytes::copy_from_slice(s.as_bytes())
}

pub(crate) async fn open(backend: &Arc<dyn SyncBackend>, name: &str) -> Rep {
    match must_finish(backend.open_replica(name)).await {
        Ok(replica) => replica,
        Err(e) => panic!("open_replica(\"{name}\") did not open the replica: {e}"),
    }
}

pub(crate) async fn open_net(backend: &Arc<dyn SyncBackend>) -> Rep {
    open(backend, REPLICA_NET).await
}

/// Reply `Ok`, or a panic describing the step.
pub(crate) fn expect_ok<T>(what: &str, res: NetResult<T>) -> T {
    match res {
        Ok(value) => value,
        Err(e) => panic!("{what}: expected Ok, got {e}"),
    }
}

/// Reply `Err(Config)`; any other outcome is a panic describing the step.
pub(crate) fn expect_config<T>(what: &str, res: NetResult<T>) {
    match res {
        Err(NetError::Config(_)) => {}
        Err(other) => panic!("{what}: expected Config, got {other}"),
        Ok(_) => panic!("{what}: expected Config, got Ok"),
    }
}

pub(crate) async fn put(r: &Rep, key: &[u8], value: &str) {
    expect_ok("set_own", must_finish(r.set_own(key, val(value))).await);
}

pub(crate) async fn get(r: &Rep, author: PeerId, key: &[u8]) -> Option<ReplicaEntry> {
    expect_ok("get", must_finish(r.get(author, key)).await)
}

pub(crate) async fn scan(r: &Rep, prefix: &[u8]) -> Vec<ReplicaEntry> {
    expect_ok("scan", must_finish(r.scan(prefix)).await)
}

pub(crate) async fn latest_per_key(r: &Rep, prefix: &[u8]) -> Vec<ReplicaEntry> {
    expect_ok("latest", must_finish(r.latest_per_key(prefix)).await)
}

/// Polls `scan(b"")` every `POLL` until `done` becomes true; panics if it has not happened within `STEP`.
pub(crate) async fn wait_scan(
    r: &Rep,
    what: &str,
    done: impl Fn(&[ReplicaEntry]) -> bool,
) -> Vec<ReplicaEntry> {
    let polling = async {
        loop {
            let entries = scan(r, b"").await;
            if done(&entries) {
                return entries;
            }
            tokio::time::sleep(POLL).await;
        }
    };
    match tokio::time::timeout(STEP, polling).await {
        Ok(entries) => entries,
        Err(_) => panic!("not reached within {STEP:?}: {what}"),
    }
}

/// Waits up to `STEP` for an event matching `want`; skips the others and `Lagged`.
pub(crate) async fn next_event(
    rx: &mut broadcast::Receiver<ReplicaEvent>,
    what: &str,
    want: impl Fn(&ReplicaEvent) -> bool,
) -> ReplicaEvent {
    let waiting = async {
        loop {
            match rx.recv().await {
                Ok(event) if want(&event) => return event,
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(e) => panic!("replica event channel closed: {e}"),
            }
        }
    };
    match tokio::time::timeout(STEP, waiting).await {
        Ok(event) => event,
        Err(_) => panic!("no event within {STEP:?}: {what}"),
    }
}

/// No `LocalChange` / `RemoteChange` arrived within `QUIET`.
pub(crate) async fn assert_no_change(rx: &mut broadcast::Receiver<ReplicaEvent>) {
    let change = |e: &ReplicaEvent| is_local(e) || is_remote(e);
    let waiting = next_event(rx, "change", change);
    let got = tokio::time::timeout(QUIET, waiting).await;
    assert!(got.is_err(), "quiet window broken by event {got:?}");
}

pub(crate) fn is_local(e: &ReplicaEvent) -> bool {
    matches!(e, ReplicaEvent::LocalChange)
}

pub(crate) fn is_remote(e: &ReplicaEvent) -> bool {
    matches!(e, ReplicaEvent::RemoteChange)
}
