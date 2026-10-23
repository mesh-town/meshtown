use super::*;
use crate::transport::net::id::NodeSecret;

fn rid() -> ReplicaId {
    ReplicaId([1; 32])
}

fn saver_in(root: &std::path::Path) -> (Saver, Arc<Mutex<State>>) {
    let path = root.join("slot").join("replica").join("net-x.bin");
    let policy = BakPolicy::FirstSave { main_ok: false };
    let me = NodeSecret::from_bytes(&[2; 32]).peer_id();
    let state = Arc::new(Mutex::new(State::new(rid(), me)));
    (Saver::new(Some(path), rid(), None, policy), state)
}

fn fresh_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("mesh-saver-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(root.join("slot")).unwrap();
    root
}

fn listing(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            out.push(p.strip_prefix(root).unwrap().display().to_string());
            if p.is_dir() {
                stack.push(p);
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn a_save_now_cancels_the_waiting_deferred_save() {
    let root = fresh_root("cancelled");
    let (saver, state) = saver_in(&root);
    saver.save_debounced(state.clone());
    saver.save_now(&state).await.unwrap();
    std::fs::rename(root.join("slot"), root.join("left")).unwrap();
    tokio::time::sleep(Duration::from_millis(1600)).await;
    let found = listing(&root);
    std::fs::remove_dir_all(&root).ok();
    assert!(!found.contains(&"slot".to_string()), "found: {found:?}");
}
