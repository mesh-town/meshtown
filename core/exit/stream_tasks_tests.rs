//! Tests of the abort-on-drop set of stream tasks.

use super::*;

#[tokio::test]
async fn drop_aborts_running_tasks() {
    // given: two tasks that never end, remembered in the set
    let (first, second) = (spawn_pending(), spawn_pending());
    let mut tasks = StreamTasks::default();
    tasks.push(first.abort_handle());
    tasks.push(second.abort_handle());

    // when: the set is dropped
    drop(tasks);

    // then: both tasks were cancelled
    for task in [first, second] {
        let joined = task.await;
        assert!(joined.err().is_some_and(|e| e.is_cancelled()), "cancelled");
    }
}

#[tokio::test]
async fn push_forgets_finished_tasks() {
    // given: a finished task remembered in the set
    let done = crate::transport::net::task_label::spawn(async {});
    let mut tasks = StreamTasks::default();
    tasks.push(done.abort_handle());
    done.await.expect("the task finished");

    // when: a new task is pushed
    let running = spawn_pending();
    tasks.push(running.abort_handle());

    // then: only the new one is kept
    assert_eq!(tasks.len(), 1);
}

fn spawn_pending() -> tokio::task::JoinHandle<()> {
    crate::transport::net::task_label::spawn(std::future::pending::<()>())
}
