use super::*;
use crate::config::config_file::Settings;
use crate::transport::net::PeerAddr;

fn secret(b: u8) -> NodeSecret {
    NodeSecret::from_bytes(&[b; 32])
}

fn switch() -> Cmd {
    Cmd::Switch {
        key: "label--0123456789abcdef".to_string(),
        peers: Vec::<PeerAddr>::new(),
    }
}

#[tokio::test]
async fn leave_from_starting_queues_leave_command() {
    let ctl = NodeCtl::new(&Settings::default());
    assert_eq!(ctl.request_leave_net(secret(1)), Ok(()));
    assert_eq!(
        ctl.next_command().await,
        Cmd::LeaveNet { secret: secret(1) }
    );
}

#[tokio::test]
async fn leave_without_network_sends_nothing() {
    let ctl = NodeCtl::new_without_network(&Settings::default());
    assert!(ctl.is_no_network());
    assert_eq!(ctl.request_leave_net(secret(1)), Ok(()));
    ctl.request_stop();
    assert_eq!(ctl.next_command().await, Cmd::Stop);
}

#[tokio::test]
async fn leave_while_stopping_is_busy() {
    let ctl = NodeCtl::new(&Settings::default());
    ctl.stop_for_leave();
    assert!(matches!(
        ctl.request_leave_net(secret(1)),
        Err(ActionError::Busy(_))
    ));
}

#[tokio::test]
async fn leave_wins_over_queued_switch() {
    let ctl = NodeCtl::new(&Settings::default());
    ctl.cmd_tx.send(switch()).unwrap();
    ctl.cmd_tx
        .send(Cmd::LeaveNet { secret: secret(2) })
        .unwrap();
    ctl.cmd_tx.send(switch()).unwrap();
    assert_eq!(
        ctl.next_command().await,
        Cmd::LeaveNet { secret: secret(2) }
    );
}

#[tokio::test]
async fn stop_wins_over_queued_leave() {
    let ctl = NodeCtl::new(&Settings::default());
    ctl.cmd_tx
        .send(Cmd::LeaveNet { secret: secret(2) })
        .unwrap();
    ctl.cmd_tx.send(Cmd::Stop).unwrap();
    ctl.cmd_tx
        .send(Cmd::LeaveNet { secret: secret(3) })
        .unwrap();
    assert_eq!(ctl.next_command().await, Cmd::Stop);
}

#[test]
fn leave_commands_compare_by_peer_id() {
    assert_eq!(
        Cmd::LeaveNet { secret: secret(1) },
        Cmd::LeaveNet { secret: secret(1) }
    );
    assert_ne!(
        Cmd::LeaveNet { secret: secret(1) },
        Cmd::LeaveNet { secret: secret(2) }
    );
    assert_ne!(Cmd::Stop, switch());
}
