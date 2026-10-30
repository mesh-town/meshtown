use super::*;

type Seen = Vec<(String, String)>;

/// Answers each request in turn with the next reply and writes the calls down.
fn run_with(
    replies: Vec<Result<Value, (u16, &str)>>,
    explicit: bool,
) -> (anyhow::Result<Created>, Seen) {
    let mut seen: Seen = Vec::new();
    let mut replies = replies.into_iter();
    let base = if explicit { "/api/net/n2" } else { "/api" };
    let result = {
        let mut call = |method: &str, path: &str, _body: Option<&Value>| {
            seen.push((method.to_string(), path.to_string()));
            match replies.next().expect("a reply for the call") {
                Ok(v) => Ok(v),
                Err((status, code)) => Err(anyhow::Error::new(ApiError::from_reply(
                    status,
                    &json!({"error": code, "message": format!("refused: {code}")}).to_string(),
                ))),
            }
        };
        create_in(&mut call, base, explicit, "ourgame")
    };
    (result, seen)
}

fn made(key: &str) -> Result<Value, (u16, &'static str)> {
    Ok(json!({"key": key, "label": "ourgame", "restart": true}))
}

fn step(method: &str, path: &str) -> (String, String) {
    (method.to_string(), path.to_string())
}

#[test]
fn free_slot_one_call() {
    let (result, seen) = run_with(vec![made("k1")], false);
    let c = result.unwrap();
    assert_eq!((c.key.as_str(), c.new_slot), ("k1", None));
    assert_eq!(seen, [step("POST", "/api/network/create")]);
}

#[test]
fn network_present_without_net_adds_a_slot() {
    let added = Ok(json!({"slot": "k3j2", "fresh": true}));
    let (result, seen) = run_with(
        vec![Err((409, "network_present")), added, made("k2")],
        false,
    );
    let c = result.unwrap();
    assert_eq!(
        (c.key.as_str(), c.new_slot.as_deref()),
        ("k2", Some("k3j2"))
    );
    assert_eq!(
        seen,
        [
            step("POST", "/api/network/create"),
            step("POST", "/api/nets"),
            step("POST", "/api/net/k3j2/network/create"),
        ]
    );
}

#[test]
fn network_present_with_net_stops_without_a_second_call() {
    let (result, seen) = run_with(vec![Err((409, "network_present"))], true);
    let text = format!("{:#}", result.unwrap_err());
    assert!(text.contains("leave out --net"), "{text}");
    assert_eq!(seen, [step("POST", "/api/net/n2/network/create")]);
}

#[test]
fn too_many_nets_stops_with_its_message() {
    let replies = vec![Err((409, "network_present")), Err((409, "too_many_nets"))];
    let (result, seen) = run_with(replies, false);
    let text = format!("{:#}", result.unwrap_err());
    assert!(text.contains("refused: too_many_nets"), "{text}");
    assert_eq!(seen.len(), 2);
}

#[test]
fn nets_answer_without_slot_is_an_error() {
    let replies = vec![Err((409, "network_present")), Ok(json!({"fresh": true}))];
    let (result, seen) = run_with(replies, false);
    assert!(result.is_err());
    assert_eq!(seen.len(), 2);
}

#[test]
fn answer_without_key_is_an_error() {
    let (result, _) = run_with(vec![Ok(json!({"label": "ourgame"}))], false);
    assert!(result.is_err());
}
