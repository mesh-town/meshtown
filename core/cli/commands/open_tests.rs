use super::*;

fn env(root: bool, display: bool, ssh: bool, desktop_os: bool) -> BrowserEnv {
    BrowserEnv {
        root,
        display,
        ssh,
        desktop_os,
    }
}

#[test]
fn browser_decision_table() {
    let table = [
        (env(false, true, false, false), true),
        (env(false, false, false, false), false),
        (env(false, false, false, true), true),
        (env(true, true, false, false), false),
        (env(true, false, false, true), false),
        (env(false, true, true, false), false),
        (env(false, false, true, true), false),
    ];
    for (e, want) in table {
        assert_eq!(may_start_browser(&e).is_ok(), want);
    }
}

#[test]
fn tail_depends_on_password() {
    assert!(tail(false).contains("10 minutes"));
    assert!(tail(true).contains("password"));
}
