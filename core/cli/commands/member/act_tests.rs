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
    assert_eq!(
        outcome(&x("a"), "pc", false),
        "Nothing changed: Admitted pc."
    );
}
