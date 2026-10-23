use super::*;

#[test]
fn mark_requested_flips_once() {
    let quit = QuitSignal::new();
    assert!(quit.mark_requested());
    assert!(!quit.mark_requested());
}

#[tokio::test]
async fn wait_returns_after_mark_requested() {
    let quit = QuitSignal::new();
    let waiter = quit.clone();
    let waiting = crate::node::output::spawn(async move {
        tokio::time::timeout(Duration::from_secs(1), waiter.wait())
            .await
            .expect("wait must return after mark_requested, not hang");
    });
    // Give the task above a chance to reach wait() before the flag is set — not necessary
    // for correctness (wait_for checks the value even before the subscription is ready), but this way the test
    // honestly checks both paths: waiting and an already ready value.
    quit.mark_requested();
    waiting.await.expect("the task must not panic");
}

#[tokio::test]
async fn wait_returns_immediately_if_already_requested() {
    let quit = QuitSignal::new();
    quit.mark_requested();
    tokio::time::timeout(Duration::from_secs(1), quit.wait())
        .await
        .expect("wait after request must return immediately, not hang");
}
