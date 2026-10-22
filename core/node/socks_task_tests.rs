use super::SocksTask;
use std::time::Duration;
use tokio::time::timeout;

/// A free port on loopback: bind with port zero, remember it, close.
pub(in crate::node) fn free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

const LIMIT: Duration = Duration::from_secs(1);

#[tokio::test]
async fn close_frees_listener_port() {
    // given: the task waits in accept on the port
    let port = free_port();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let task = SocksTask::new(tokio::spawn(async move {
        let _ = listener.accept().await;
    }));
    // when: close
    task.close().await;
    // then: the port is free at once
    assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_ok(), "port {port} is busy after close");
}

#[tokio::test]
async fn close_without_task_returns() {
    assert!(timeout(LIMIT, SocksTask::default().close()).await.is_ok());
}

#[tokio::test]
async fn second_close_through_clone_returns() {
    let task = SocksTask::new(tokio::spawn(std::future::pending::<()>()));
    let clone = task.clone();
    task.close().await;
    assert!(timeout(LIMIT, clone.close()).await.is_ok());
}

#[tokio::test]
async fn close_after_task_finished_returns() {
    let handle = tokio::spawn(async {});
    while !handle.is_finished() {
        tokio::task::yield_now().await;
    }
    let task = SocksTask::new(handle);
    assert!(timeout(LIMIT, task.close()).await.is_ok());
}
