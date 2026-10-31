use super::*;

fn exclude(reason: Option<&str>) -> MemberCmd {
    MemberCmd::Exclude {
        member: "x".into(),
        reason: reason.map(str::to_string),
    }
}

fn admin(revoke: bool) -> MemberCmd {
    MemberCmd::Admin {
        member: "x".into(),
        revoke,
    }
}

#[test]
fn exclude_body_with_and_without_reason() {
    assert_eq!(
        action_body(&exclude(Some(" gone ")), "ab"),
        json!({"id": "ab", "reason": "gone"})
    );
    assert_eq!(action_body(&exclude(None), "ab"), json!({"id": "ab"}));
    let admit = MemberCmd::Admit { member: "x".into() };
    assert_eq!(action_body(&admit, "ab"), json!({"id": "ab"}));
}

#[test]
fn blank_reason_dropped() {
    assert_eq!(check_reason(Some("   ")).unwrap(), None);
    assert_eq!(check_reason(None).unwrap(), None);
    assert_eq!(check_reason(Some(" why ")).unwrap().as_deref(), Some("why"));
    assert_eq!(action_body(&exclude(Some("  ")), "ab"), json!({"id": "ab"}));
}

#[test]
fn reason_over_80_is_usage() {
    assert!(check_reason(Some(&"a".repeat(MAX_REASON_CHARS))).is_ok());
    let err = check_reason(Some(&"a".repeat(MAX_REASON_CHARS + 1))).unwrap_err();
    assert!(err.0.contains("at most 80"), "{}", err.0);
    assert_eq!(MAX_REASON_CHARS, 80);
}

#[test]
fn reason_control_char_is_usage() {
    for bad in ["a\u{1b}b", "a\nb", "a\n", "\tz"] {
        assert!(check_reason(Some(bad)).is_err(), "{bad:?}");
    }
}

#[test]
fn admin_body_grant_revoke() {
    assert_eq!(
        action_body(&admin(false), "ab"),
        json!({"id": "ab", "admin": true})
    );
    assert_eq!(
        action_body(&admin(true), "ab"),
        json!({"id": "ab", "admin": false})
    );
}

#[test]
fn outcome_per_action_and_unchanged() {
    let x = |s: &str| MemberCmd::Admit { member: s.into() };
    assert_eq!(outcome(&x("a"), "pc (1)", true), "Admitted pc (1).");
    assert_eq!(
        outcome(&exclude(None), "pc", true),
        "Removed pc from the network."
    );
    let un = MemberCmd::Unexclude { member: "a".into() };
    assert_eq!(outcome(&un, "pc", true), "Withdrew the removal of pc.");
    let b = MemberCmd::Block { member: "a".into() };
    assert_eq!(outcome(&b, "pc", true), "Blocked pc on this computer.");
    let ub = MemberCmd::Unblock { member: "a".into() };
    assert_eq!(outcome(&ub, "pc", true), "Unblocked pc.");
    assert_eq!(
        outcome(&admin(false), "pc", true),
        "pc is now an administrator."
    );
    assert_eq!(
        outcome(&admin(true), "pc", true),
        "pc is no longer an administrator."
    );
}

fn unchanged(cmd: &MemberCmd) -> String {
    outcome(cmd, "pc", false)
}

#[test]
fn outcome_unchanged_says_already_admitted_or_removed() {
    let admit = MemberCmd::Admit { member: "a".into() };
    assert_eq!(
        unchanged(&admit),
        "Nothing changed: pc is already admitted."
    );
    assert_eq!(
        unchanged(&exclude(None)),
        "Nothing changed: pc is already removed from the network."
    );
    let un = MemberCmd::Unexclude { member: "a".into() };
    assert_eq!(unchanged(&un), "Nothing changed: pc is not removed.");
}

#[test]
fn outcome_unchanged_says_already_blocked_or_not() {
    let b = MemberCmd::Block { member: "a".into() };
    assert_eq!(unchanged(&b), "Nothing changed: pc is already blocked.");
    let ub = MemberCmd::Unblock { member: "a".into() };
    assert_eq!(unchanged(&ub), "Nothing changed: pc is not blocked.");
}

#[test]
fn outcome_unchanged_says_already_administrator_or_not() {
    assert_eq!(
        unchanged(&admin(false)),
        "Nothing changed: pc is already an administrator."
    );
    assert_eq!(
        unchanged(&admin(true)),
        "Nothing changed: pc is not an administrator."
    );
}
