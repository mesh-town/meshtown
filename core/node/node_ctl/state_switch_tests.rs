use super::*;
use crate::transport::net::NodeSecret;

fn switch(key: &str, secret: Option<NodeSecret>) -> Cmd {
    Cmd::Switch {
        key: key.to_string(),
        peers: vec![],
        secret,
    }
}

fn secret_of(cmd: Cmd) -> (String, Option<crate::transport::net::PeerId>) {
    match cmd {
        Cmd::Switch { key, secret, .. } => (key, secret.map(|s| s.peer_id())),
        other => panic!("expected a switch, got {other:?}"),
    }
}

#[tokio::test]
async fn merged_switches_keep_the_new_secret() {
    // given: a switch with a new node key, then a plain one (a restart)
    let ctl = NodeCtl::new(&Settings::default());
    let new = NodeSecret::from_bytes(&[77u8; 32]);
    ctl.cmd_tx.send(switch("early", Some(new.clone()))).unwrap();
    ctl.cmd_tx.send(switch("late", None)).unwrap();
    // when
    let got = ctl.next_command().await;
    // then: the later network key, the earlier node key
    assert_eq!(secret_of(got), ("late".to_string(), Some(new.peer_id())));
}

#[tokio::test]
async fn merged_switches_take_the_later_secret() {
    let ctl = NodeCtl::new(&Settings::default());
    ctl.cmd_tx.send(switch("a", None)).unwrap();
    ctl.cmd_tx.send(switch("b", None)).unwrap();
    assert_eq!(secret_of(ctl.next_command().await), ("b".to_string(), None));
    let late = NodeSecret::from_bytes(&[78u8; 32]);
    ctl.cmd_tx.send(switch("c", None)).unwrap();
    ctl.cmd_tx.send(switch("d", Some(late.clone()))).unwrap();
    assert_eq!(
        secret_of(ctl.next_command().await),
        ("d".to_string(), Some(late.peer_id()))
    );
}
