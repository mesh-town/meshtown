use super::*;

#[test]
fn title_then_body_after_app_name() {
    let args = args("The title", "The body");

    assert_eq!(
        args,
        ["--app-name", APP_NAME, "--", "The title", "The body"]
    );
}
