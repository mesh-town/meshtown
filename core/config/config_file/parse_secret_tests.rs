//! Tests for a corrupted identity secret, split out of `parse_tests.rs`.

use super::*;

mod message_for_the_member {
    use super::*;

    #[test]
    fn explains_the_new_identity_in_terms_of_computers() {
        // given: a secret that is not base64
        let p = parse_with_secret(r#""!!!junk-value""#);
        // when
        let message = only_error_message(&p);
        // then: the whole text, with the reason in the brackets
        assert_eq!(
            message,
            "identity secret is corrupted (not base64); this computer has a new identity, \
             other members will see it as a new computer"
        );
    }

    #[test]
    fn names_the_wrong_length_as_the_reason() {
        // given: a valid base64 string with 16 bytes inside
        let b64 = data_encoding::BASE64.encode(&[1u8; 16]);
        let p = parse_with_secret(&format!(r#""{b64}""#));
        // when
        let message = only_error_message(&p);
        // then
        assert!(
            message.contains("identity secret is corrupted (16 bytes instead of 32); "),
            "{message}"
        );
        assert!(
            message.contains("this computer has a new identity"),
            "{message}"
        );
    }

    #[test]
    fn names_the_wrong_kind_as_the_reason() {
        // given: the secret is a number
        let p = parse_with_secret("5");
        // when
        let message = only_error_message(&p);
        // then
        assert!(
            message.contains("identity secret is corrupted (not a string); "),
            "{message}"
        );
    }

    #[test]
    fn does_not_call_the_computer_a_node() {
        for secret in [r#""!!!junk-value""#, "5", "null"] {
            // given: each kind of damage
            let p = parse_with_secret(secret);
            // when
            let message = only_error_message(&p);
            // then: the user-facing word is "computer"
            assert!(
                !message.to_lowercase().contains("node"),
                "secret = {secret}: {message}"
            );
            assert!(
                message.contains("this computer has a new identity"),
                "secret = {secret}: {message}"
            );
        }
    }
}

mod message_for_the_section_that_is_not_an_object {
    use super::*;

    #[test]
    fn has_the_same_ending() {
        // given: `identity` is a number
        let text = r#"{"identity": 7, "config": {"version": 1}}"#;
        let p = parse_text(text).expect("parse");
        // when
        let message = only_error_message(&p);
        // then
        assert!(
            message.starts_with(
                "identity secret is corrupted (the identity section is not an object); "
            ),
            "{message}"
        );
        assert!(
            message.ends_with("other members will see it as a new computer"),
            "{message}"
        );
    }
}

fn parse_with_secret(secret_json: &str) -> ParsedConfigFile {
    let text =
        format!(r#"{{"identity": {{"secret": {secret_json}}}, "config": {{"version": 1}}}}"#);
    parse_text(&text).expect("the document parses")
}

fn only_error_message(p: &ParsedConfigFile) -> &str {
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    assert_eq!(p.field_errors[0].field_path, "identity.secret");
    &p.field_errors[0].message
}
