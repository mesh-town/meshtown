//! Tests of `Counted` and `wait_second_half` (`splice_progress.rs`).

use super::*;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn counted_adds_only_accepted_bytes() {
    // given: a writer that accepts at most 4 bytes in total
    let (inner, _peer) = tokio::io::duplex(4);
    let progress = Progress::default();
    let mut counted = Counted::new(inner, progress.clone());

    // when: 10 bytes are offered once (a partial write)
    let accepted = counted.write(&[1u8; 10]).await.expect("write");

    // then: only the accepted ones are counted
    assert_eq!(accepted, 4);
    assert_eq!(progress.seen(), 4);
}

#[tokio::test(start_paused = true)]
async fn wait_second_half_returns_zero_when_nothing_moves() {
    // given: a task that never ends and a progress that never changes
    let mut task: JoinHandle<u64> = crate::node::output::spawn(std::future::pending());

    // when: waiting after a closed half with traffic
    let got = wait_second_half(&mut task, &Progress::default(), 1).await;

    // then: zero, and the task is left to the caller to abort
    assert_eq!(got, 0);
    task.abort();
}
