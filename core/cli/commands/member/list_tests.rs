use serde_json::{json, Value};

use super::*;

fn row_of(extra: Value) -> Vec<String> {
    let mut m = json!({
        "id": "aaaa111122", "id_full": "aaaa111122223333", "name": "laptop", "self": false,
        "status": "member", "online": true, "admin": false, "blocked_by_me": false,
    });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    rows(&json!({"members": [m]})).remove(0)
}

fn row_with_reason(me: Value, extra: Value) -> Vec<String> {
    let mut m = json!({
        "id": "aaaa111122", "name": "laptop", "self": false,
        "status": "excluded", "online": false, "admin": false, "blocked_by_me": false,
    });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    let mut reply = json!({"members": [m]});
    for (k, v) in me.as_object().unwrap() {
        reply[k] = v.clone();
    }
    rows(&reply).remove(0)
}

fn admin() -> Value {
    json!({"me_admin": true, "me_excluded": false})
}

#[test]
fn name_last_keeps_spaces() {
    let r = row_of(json!({"name": "my  old laptop"}));
    assert_eq!(
        r,
        ["aaaa111122", "member", "yes", "no", "no", "my  old laptop"]
    );
}

#[test]
fn null_name_is_dash() {
    assert_eq!(row_of(json!({"name": null}))[5], "-");
    assert_eq!(row_of(json!({"name": ""}))[5], "-");
}

#[test]
fn self_marker() {
    assert_eq!(row_of(json!({"self": true}))[5], "laptop (this computer)");
    assert_eq!(
        row_of(json!({"self": true, "name": null}))[5],
        "- (this computer)"
    );
}

#[test]
fn control_chars_replaced() {
    let name = row_of(json!({"name": "\u{1b}[31mred\nline"}))[5].clone();
    assert!(!name.chars().any(char::is_control), "{name:?}");
    assert_eq!(name, "?[31mred?line");
}

#[test]
fn flags_yes_no() {
    let r =
        row_of(json!({"online": false, "admin": true, "blocked_by_me": true, "status": "pending"}));
    assert_eq!(r[1..5], ["pending", "no", "yes", "yes"]);
}

#[test]
fn empty_gives_no_rows() {
    assert!(rows(&json!({"members": []})).is_empty());
    assert!(rows(&Value::Null).is_empty());
}

fn reply(me_admin: bool, statuses: &[&str]) -> Value {
    let members: Vec<Value> = statuses
        .iter()
        .map(|s| json!({"status": s, "online": true}))
        .collect();
    json!({"me_admin": me_admin, "me_pending": false, "me_excluded": false, "members": members})
}

#[test]
fn hints_admit_only_for_admin_with_pending() {
    let admin = hints(&reply(true, &["member", "pending", "pending"]));
    assert_eq!(admin[0], "1 member (1 online)");
    assert!(admin[1].starts_with("2 waiting: run `"), "{admin:?}");
    assert!(
        admin[1].ends_with("member admit <name or id>`"),
        "{admin:?}"
    );
    assert_eq!(hints(&reply(false, &["member", "pending"])).len(), 1);
    assert_eq!(hints(&reply(true, &["member"])).len(), 1);
}

#[test]
fn hints_for_pending_and_excluded_self() {
    let mut r = reply(false, &["member", "member"]);
    r["me_pending"] = json!(true);
    r["me_excluded"] = json!(true);
    let lines = hints(&r);
    assert_eq!(lines[0], "2 members (2 online)");
    assert!(lines[1].contains("waits for an administrator"), "{lines:?}");
    assert!(lines[2].contains("removed this computer"), "{lines:?}");
}

#[test]
fn reason_column_for_removed_member() {
    let r = row_with_reason(
        admin(),
        json!({"excluded": {"how": "removed", "reason": "spam"}}),
    );
    assert_eq!(r.len(), 7);
    assert_eq!(r[6], "spam");
    let reply = json!({"me_admin": true, "members": []});
    assert_eq!(head(&reply).last(), Some(&"REASON"));
}

#[test]
fn reason_column_dash_for_others() {
    let r = row_with_reason(admin(), json!({"status": "member"}));
    assert_eq!(r[6], "-");
}

#[test]
fn reason_column_not_given() {
    let r = row_with_reason(
        admin(),
        json!({"excluded": {"how": "removed", "reason": null}}),
    );
    assert_eq!(r[6], "(not given)");
}

#[test]
fn esc_in_reason_is_cleaned_in_the_table() {
    let r = row_with_reason(
        admin(),
        json!({"excluded": {"how": "removed", "reason": "\u{1b}]0;x\u{7}\u{202e}"}}),
    );
    assert!(
        !r[6].contains('\u{1b}') && !r[6].contains('\u{7}'),
        "{:?}",
        r[6]
    );
    assert!(!r[6].contains('\u{202e}'), "{:?}", r[6]);
}

#[test]
fn excluded_self_sees_the_column() {
    let me = json!({"me_admin": false, "me_excluded": true});
    let r = row_with_reason(
        me.clone(),
        json!({"self": true, "excluded": {"how": "removed", "reason": "spam"}}),
    );
    assert_eq!(r[6], "spam");
    assert_eq!(head(&me).len(), 7);
}

#[test]
fn reason_column_dash_for_a_left_member() {
    let r = row_with_reason(admin(), json!({"excluded": {"how": "left"}}));
    assert_eq!(r[6], "-");
    let r = row_with_reason(
        admin(),
        json!({"excluded": {"how": "left", "reason": "stale"}}),
    );
    assert_eq!(r[6], "-");
    let r = row_with_reason(admin(), json!({"excluded": {"how": "removed"}}));
    assert_eq!(r[6], "-");
}

#[test]
fn no_reason_column_for_a_plain_member() {
    let me = json!({"me_admin": false, "me_excluded": false});
    let r = row_with_reason(me.clone(), json!({"status": "member"}));
    assert_eq!(r.len(), 6);
    assert_eq!(
        head(&me),
        ["ID", "STATUS", "ONLINE", "ADMIN", "BLOCKED", "NAME"]
    );
}
