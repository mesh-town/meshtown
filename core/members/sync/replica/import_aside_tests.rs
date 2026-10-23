//! Tests for `import.rs`: `docs-store/` is moved aside instead of failing the start.

use super::tests::{node, run, state_of};
use super::*;

async fn run_for_seed(dir: &Path, seed: &[u8; 32]) -> NetResult<()> {
    import_docs_store(Some(dir), &super::tests::secret(), seed).await
}

fn parent(dir: &Path) -> &Path {
    dir.parent().unwrap()
}

#[tokio::test]
async fn docs_store_is_set_aside() {
    let dir = node(true);
    run(&dir).await.unwrap();
    assert!(!dir.exists());
    assert!(parent(&dir).join("docs-store.old").join("docs").exists());
    for name in ["net", "domains"] {
        assert_eq!(state_of(&dir, name).0.unwrap().source, MarkSource::Absent);
    }
}

#[test]
fn old_taken_gets_seconds_suffix() {
    let dir = node(true);
    let old = parent(&dir).join("docs-store.old");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("marker"), b"x").unwrap();
    let to = set_aside(&dir, 1_700_000_000).unwrap();
    assert_eq!(to, parent(&dir).join("docs-store.old-1700000000"));
    assert!(old.join("marker").exists());
    assert!(to.join("docs").exists());
}

#[test]
fn old_and_seconds_taken_gets_counter() {
    let dir = node(true);
    for name in ["docs-store.old", "docs-store.old-1700000000"] {
        std::fs::create_dir_all(parent(&dir).join(name)).unwrap();
    }
    let to = set_aside(&dir, 1_700_000_000).unwrap();
    assert_eq!(to, parent(&dir).join("docs-store.old-1700000000-2"));
}

#[tokio::test]
async fn second_network_after_aside_ok() {
    let dir = node(true);
    run(&dir).await.unwrap();
    let seed = [9u8; 32];
    run_for_seed(&dir, &seed).await.unwrap();
    assert!(!dir.exists());
    let id = net_id(&seed);
    let path = file::path_for(&file::replica_dir(&dir).unwrap(), "net", &id);
    let Loaded::Main(mark, _) = file::load(&path, &id).unwrap() else {
        panic!("the file of network 9 was not written")
    };
    assert_eq!(mark.unwrap().source, MarkSource::Absent);
}

#[cfg(unix)]
#[test]
fn rename_failure_is_config_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = node(true);
    let up = parent(&dir).to_path_buf();
    std::fs::set_permissions(&up, std::fs::Permissions::from_mode(0o555)).unwrap();
    let res = set_aside(&dir, 1);
    std::fs::set_permissions(&up, std::fs::Permissions::from_mode(0o755)).unwrap();
    let text = match res {
        Err(NetError::Config(t)) => t,
        other => panic!("expected Config, got {other:?}"),
    };
    assert!(text.contains("by hand"), "{text}");
    assert!(dir.join("docs").exists());
}

#[tokio::test]
async fn marked_keeps_docs_store() {
    // given: both replicas are marked, a new docs-store/ appears
    let dir = node(true);
    run(&dir).await.unwrap();
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    // when
    run(&dir).await.unwrap();
    // then: the directory stays where it is
    assert!(dir.join("docs").exists());
}
