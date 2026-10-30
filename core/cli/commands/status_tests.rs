use serde_json::{json, Value};

use super::*;

fn two_nets() -> Vec<Value> {
    vec![
        json!({"id": "main", "label": "home", "id_hex": "aa", "status": "online",
               "peers_online": 2, "peers_total": 3, "exit_enabled": true, "proxy": true}),
        json!({"id": "k3j2h4", "label": "work", "id_hex": "bb", "status": "connecting",
               "peers_online": 0, "peers_total": 1, "exit_enabled": false, "proxy": false}),
    ]
}

#[test]
fn lines_for_two_nets_with_exit_and_proxy() {
    let lines = status_lines(&two_nets());
    assert_eq!(lines[0], "home\tonline\t2/3 online\texit\tproxy");
    assert_eq!(lines[1], "work\tconnecting\t0/1 online");
}

#[test]
fn line_for_node_without_network() {
    let net = json!({"id": "main", "label": null, "id_hex": null, "status": "no_key",
                     "peers_online": 0, "peers_total": 0});
    assert_eq!(status_lines(&[net]), ["(no network)\tno_key\t0/0 online"]);
}

fn nets_reply(status: &str, id_hex: Value) -> Value {
    json!({"nets": [{"id": "main", "label": "home", "id_hex": id_hex, "status": status}]})
}

#[test]
fn wait_step_online_is_done() {
    let nets = nets_reply("online", json!("aa"));
    assert_eq!(wait_step(Ok(&nets), None), WaitStep::Done);
}

#[test]
fn wait_step_offline_waits() {
    let nets = nets_reply("connecting", json!("aa"));
    assert_eq!(wait_step(Ok(&nets), None), WaitStep::Wait);
}

#[test]
fn wait_step_without_network_fails() {
    let nets = nets_reply("no_key", Value::Null);
    assert_eq!(
        wait_step(Ok(&nets), None),
        WaitStep::Fail("no network to wait for".into())
    );
}

#[test]
fn wait_step_not_running_waits() {
    let err = anyhow::Error::new(NotRunning { port: 7777 }).context("looking");
    assert_eq!(wait_step(Err(&err), None), WaitStep::Wait);
}

#[test]
fn wait_step_other_error_fails() {
    let err = anyhow::anyhow!("boom");
    assert_eq!(wait_step(Err(&err), None), WaitStep::Fail("boom".into()));
}

#[test]
fn wait_step_follows_the_chosen_slot() {
    let nets = json!({"nets": two_nets()});
    assert_eq!(wait_step(Ok(&nets), Some("k3j2h4")), WaitStep::Wait);
    assert_eq!(wait_step(Ok(&nets), Some("main")), WaitStep::Done);
}

#[test]
fn shown_nets_keeps_only_the_chosen_slot() {
    let overview = json!({"nets": two_nets()});
    assert_eq!(shown_nets(&overview, Some("k3j2h4")).len(), 1);
    assert_eq!(shown_nets(&overview, None).len(), 2);
}
