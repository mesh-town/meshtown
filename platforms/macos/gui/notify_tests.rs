use super::*;

#[test]
fn texts_are_argv_not_script() {
    let title = r#"-t "q" \"#;
    let body = r#"say "hi" \ again"#;

    let args = args(title, body);

    let (script, texts) = args.split_at(args.len() - 2);
    assert_eq!(texts, [title, body], "the texts go verbatim, last");
    assert_eq!(texts[0], title, "the title comes first");
    assert_eq!(texts[1], body, "the body comes second");
    let joined = script.join("\n");
    assert!(
        joined.contains("with title (item 1 of argv)"),
        "the script reads the title as item 1"
    );
    assert!(
        joined.contains("display notification (item 2 of argv)"),
        "the script reads the body as item 2"
    );
    assert_eq!(script.last(), Some(&"--"), "`--` ends the options");
    for part in script {
        assert!(
            !part.contains(title) && !part.contains(body),
            "a text leaked into the script: {part}"
        );
    }
}
