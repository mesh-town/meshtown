use super::*;
use crate::config::config_file::FileState;

fn report(persistent: bool, declined: bool, refused: bool) -> FileReport {
    FileReport {
        path: None,
        state: FileState::Ok,
        writable: true,
        message: None,
        field_errors: vec![],
        rev: 0,
        persistent_identity: persistent,
        identity_declined: declined,
        overwrite_refused: refused,
    }
}

/// Parsing order: `persistent_identity` → `identity_declined` →
/// `overwrite_refused` → otherwise `FileProblem`.
#[test]
fn identity_from_report_order() {
    assert_eq!(
        IdentityState::from_report(&report(true, false, false)),
        IdentityState::Persistent
    );
    assert_eq!(
        IdentityState::from_report(&report(false, true, false)),
        IdentityState::Temporary(TemporaryReason::Declined)
    );
    assert_eq!(
        IdentityState::from_report(&report(false, false, true)),
        IdentityState::Temporary(TemporaryReason::FilePreserved)
    );
    assert_eq!(
        IdentityState::from_report(&report(false, false, false)),
        IdentityState::Temporary(TemporaryReason::FileProblem)
    );
}

#[test]
fn identity_codes() {
    assert_eq!(IdentityState::Persistent.kind_code(), "persistent");
    assert_eq!(IdentityState::Persistent.temporary_reason_code(), None);
    assert_eq!(IdentityState::Persistent.kind_word(), "persistent");
    assert_eq!(IdentityState::Persistent.warning_text(), None);

    let declined = IdentityState::Temporary(TemporaryReason::Declined);
    assert_eq!(declined.kind_code(), "temporary");
    assert_eq!(declined.temporary_reason_code(), Some("declined"));
    assert_eq!(declined.kind_word(), "temporary");

    let preserved = IdentityState::Temporary(TemporaryReason::FilePreserved);
    assert_eq!(preserved.kind_code(), "temporary");
    assert_eq!(preserved.temporary_reason_code(), Some("file_preserved"));
    assert_eq!(preserved.kind_word(), "temporary");

    let problem = IdentityState::Temporary(TemporaryReason::FileProblem);
    assert_eq!(problem.kind_code(), "temporary");
    assert_eq!(problem.temporary_reason_code(), Some("file_problem"));
    assert_eq!(problem.kind_word(), "temporary");
}

/// The page (`web_gui/ui/vue/src/shell/shell-texts.ts`) describes a temporary identity in its own words
/// for people, while the node in `identity.rs` uses its own, for the log. The test holds
/// the page strictly: each reason and the shared consequence (the `CONSEQUENCE` template) must appear in
/// the source verbatim; editing any page phrase breaks the test until the expectation is updated.
/// The network name inside a phrase is the splice `" + NET_NAME + "`, the test substitutes the real name. The phrases about
/// remembering the computer (`Declined`, `FileProblem`) start with the name: in the source this is the splice
/// `NET_NAME + " doesn't …"`, the test looks for it verbatim, and the name in it stays an identifier.
/// The node texts are checked only for being non-empty for each reason.
#[test]
fn identity_texts_match_page() {
    let path = format!(
        "{}/web_gui/ui/vue/src/shell/shell-texts.ts",
        env!("CARGO_MANIFEST_DIR")
    );
    let page = std::fs::read_to_string(&path)
        .expect("web_gui/ui/vue/src/shell/shell-texts.ts is readable")
        .replace("\" + NET_NAME + \"", crate::node::output::NET_NAME);
    let consequence = "After a restart, other members will see \" + who + \" as a new member.";
    assert!(
        page.contains(consequence),
        "the shared consequence text was not found in shell/shell-texts.ts"
    );
    let page_texts = [
        (TemporaryReason::Declined, "NET_NAME + \" was started in a mode that doesn't remember this computer. \"".to_string()),
        (TemporaryReason::FilePreserved, format!("The settings file couldn't be updated, and the node left it as is so that nothing is lost. Don't delete this file: it may contain access to your {} network. ", crate::node::output::NET_NAME)),
        (TemporaryReason::FileProblem, "NET_NAME + \" couldn't remember this computer: there is an error in the settings file. \"".to_string()),
    ];
    for (reason, expected) in page_texts {
        assert!(
            page.contains(&expected),
            "text {reason:?} was not found verbatim in shell/shell-texts.ts: {expected}"
        );
        assert!(IdentityState::Temporary(reason)
            .warning_text()
            .is_some_and(|t| !t.is_empty()));
    }
}
