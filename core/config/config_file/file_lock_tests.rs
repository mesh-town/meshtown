use super::*;
use crate::config::config_file::test_support::scratch;

#[test]
fn second_holder_times_out_then_gets_the_lock_after_drop() {
    let dir = scratch("lock");
    let path = dir.join("x.lock");
    let first = FileLock::exclusive(&path, Duration::from_secs(1))
        .unwrap()
        .expect("locks are supported");
    let err = FileLock::exclusive(&path, Duration::from_millis(50)).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    drop(first);
    let second = FileLock::exclusive(&path, Duration::from_secs(1)).unwrap();
    assert!(second.is_some());
}
