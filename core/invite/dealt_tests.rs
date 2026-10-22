use super::*;
use crate::transport::net::NodeSecret;
use serde_json::json;

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn dealt_with(count: u8) -> Dealt {
    Dealt {
        text: "mesh-invite-SECRET".into(),
        holders: (1..=count).map(id).collect(),
    }
}

#[test]
fn debug_of_dealt_hides_the_invite_text() {
    let dealt = dealt_with(1);
    let shown = format!("{dealt:?}");
    assert!(!shown.contains(&dealt.text), "{shown}");
    assert!(!shown.contains("SECRET"), "{shown}");
}

#[test]
fn api_body_has_invite_holders_and_reach_for_every_reach() {
    let expected = [(0, "issuer_only"), (1, "issuer_and_one"), (2, "spread")];
    for (count, reach) in expected {
        let dealt = dealt_with(count);
        let body = dealt.api_body();
        assert_eq!(
            body,
            json!({"invite": dealt.text, "holders": count, "reach": reach})
        );
    }
}

#[test]
fn console_line_for_issuer_only_says_only_this_node() {
    assert!(Reach::IssuerOnly
        .console_line()
        .contains("only this node will let a newcomer in"));
}

#[test]
fn console_line_for_spread_says_works_without_this_node() {
    assert!(Reach::Spread
        .console_line()
        .contains("even without this node"));
}
