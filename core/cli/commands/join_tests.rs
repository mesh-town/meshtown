use super::*;
use serde_json::json;

const NET: &str = "9c1e55aa";

fn state_with(join: Value) -> Value {
    json!({"node": {"invite_join": join}})
}

fn nets_with(id: &str, id_hex: &str) -> Value {
    json!({"nets": [{"id": id, "id_hex": id_hex}]})
}

fn refusal(status: u16, code: &str) -> anyhow::Error {
    let body = json!({"error": code, "message": "no"}).to_string();
    anyhow::Error::new(ApiError::from_reply(status, &body))
}

#[test]
fn poll_waiting_waits() {
    // given: the node is still waiting for members
    let state = state_with(json!({"state": "waiting", "message": "…"}));
    // when / then
    assert_eq!(
        poll_step(&state, &json!({"nets": []}), "main", NET),
        PollStep::Wait
    );
}

#[test]
fn poll_refused_fails_with_message() {
    // given: a final refusal
    let state = state_with(json!({"state": "refused", "message": "the invite is spent"}));
    // when / then
    let step = poll_step(&state, &json!({"nets": []}), "main", NET);
    assert_eq!(step, PollStep::Fail("the invite is spent".to_string()));
}

#[test]
fn poll_null_with_matching_net_is_joined() {
    // given: the join state is gone and the slot holds the invite's network
    let nets = nets_with("main", "9c1e55aa00112233");
    // when / then
    assert_eq!(
        poll_step(&state_with(Value::Null), &nets, "main", NET),
        PollStep::Joined
    );
}

#[test]
fn poll_null_with_other_net_is_cancelled() {
    // given: the join state is gone but the slot holds another network
    let nets = nets_with("main", "ffffffff00112233");
    // when
    let step = poll_step(&state_with(Value::Null), &nets, "main", NET);
    // then
    let text = "the join was cancelled (another join or a stop)".to_string();
    assert_eq!(step, PollStep::Fail(text));
}

#[test]
fn live_answer_network_present_adds_slot() {
    assert_eq!(
        live_answer(&refusal(409, "network_present")),
        LiveAnswer::AddSlot
    );
}

#[test]
fn live_answer_already_added_is_success() {
    assert_eq!(
        live_answer(&refusal(409, "net_already_added")),
        LiveAnswer::Already
    );
}

#[test]
fn live_answer_other_refusal_fails() {
    assert_eq!(live_answer(&refusal(422, "invalid_key")), LiveAnswer::Fail);
    assert_eq!(live_answer(&anyhow::anyhow!("plain")), LiveAnswer::Fail);
}

#[test]
fn network_listed_finds_a_slot_with_the_network() {
    let nets =
        json!({"nets": [{"id": "main", "id_hex": null}, {"id": "x1", "id_hex": "9c1e55aa"}]});
    assert!(network_listed(&nets, "9c1e55aa0011"));
    assert!(!network_listed(&nets, "aaaaaaaa0011"));
}

#[test]
fn network_listed_ignores_empty_slots() {
    let nets = json!({"nets": [{"id": "main", "id_hex": null}, {"id": "x1", "id_hex": ""}]});
    assert!(!network_listed(&nets, "9c1e55aa"));
    assert!(!network_listed(&json!({"nets": []}), "9c1e55aa"));
}

#[test]
fn unreadable_text_is_a_usage_error_with_an_invite_hint() {
    let e = parse_text("garbage string").err().expect("not a key");
    assert!(e.downcast_ref::<UsageError>().is_some());
    let text = e.to_string();
    assert!(text.contains("invite"), "{text}");
    assert!(!text.contains("keygen"), "{text}");
    assert!(!text.contains('\n'), "{text}");
}
