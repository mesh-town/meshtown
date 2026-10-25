//! The process runtime: the output of the body and the bounded wait for blocking threads on exit.

use std::time::{Duration, Instant};

use super::*;

#[test]
fn returns_the_body_output() {
    let out = run_to_exit(async { 42u8 });

    assert_eq!(out, 42);
}

#[test]
fn stuck_blocking_task_does_not_hold_the_exit() {
    // given: a body that leaves a blocking thread asleep for much longer than the drain
    let body = async {
        let (running, started) = tokio::sync::oneshot::channel();
        crate::transport::net::task_label::spawn_blocking(move || {
            let _ = running.send(());
            std::thread::sleep(Duration::from_secs(10));
        });
        started.await.expect("the blocking task starts");
    };
    let began = Instant::now();

    // when
    run_to_exit(body);

    // then: the exit did not wait for that thread
    let took = began.elapsed();
    assert!(
        took < EXIT_DRAIN + Duration::from_secs(2),
        "the exit took {took:?}"
    );
}
