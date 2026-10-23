//! Tests of the label transfer: `with_net` and the spawn wrappers.

use super::*;

fn label(id: &str) -> Option<Arc<str>> {
    Some(Arc::from(id))
}

fn text(l: Option<Arc<str>>) -> Option<String> {
    l.map(|n| n.to_string())
}

#[tokio::test]
async fn label_is_visible_inside_with_net_only() {
    assert!(current_net().is_none());
    let got = with_net(label("ab2c3d"), async { current_net() }).await;
    assert_eq!(text(got).as_deref(), Some("ab2c3d"));
    assert!(current_net().is_none());
}

#[tokio::test]
async fn spawn_carries_the_label() {
    let got = with_net(label("n1"), async {
        spawn(async { current_net() }).await.unwrap()
    })
    .await;
    assert_eq!(text(got).as_deref(), Some("n1"));
}

#[tokio::test]
async fn spawn_blocking_carries_the_label() {
    let got = with_net(label("n2"), async {
        spawn_blocking(current_net).await.unwrap()
    })
    .await;
    assert_eq!(text(got).as_deref(), Some("n2"));
}

#[tokio::test]
async fn spawn_in_carries_the_label() {
    let mut set = JoinSet::new();
    with_net(label("n3"), async {
        spawn_in(&mut set, async { current_net() });
    })
    .await;
    let got = set.join_next().await.unwrap().unwrap();
    assert_eq!(text(got).as_deref(), Some("n3"));
}

#[tokio::test]
async fn spawn_on_carries_the_label() {
    let handle = Handle::current();
    let got = with_net(label("n4"), async move {
        spawn_on(&handle, async { current_net() }).await.unwrap()
    })
    .await;
    assert_eq!(text(got).as_deref(), Some("n4"));
}
