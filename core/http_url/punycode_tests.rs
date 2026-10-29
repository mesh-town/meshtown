use super::*;

#[test]
fn encodes_rfc3492_samples() {
    // Labels are already NFKC and lower case, as the function expects.
    for (label, want) in [
        ("b\u{FC}cher", "bcher-kva"),
        ("m\u{FC}nchen", "mnchen-3ya"),
        ("\u{4F8B}\u{3048}", "r8jz45g"),
        ("\u{30C6}\u{30B9}\u{30C8}", "zckzah"),
        ("\u{D55C}\u{AD6D}\u{C5B4}", "3e0bk47br7k"),
        ("\u{3B5}\u{3BB}\u{3BB}\u{3AC}\u{3B4}\u{3B1}", "hxakic4aa"),
        // RFC 3492 section 7.1, sample (A): Arabic (Egyptian).
        (
            "\u{0644}\u{064A}\u{0647}\u{0645}\u{0627}\u{0628}\u{062A}\u{0643}\u{0644}\u{0645}\u{0648}\u{0634}\u{0639}\u{0631}\u{0628}\u{064A}\u{061F}",
            "egbpdaj6bu4bxfgehfvwxn",
        ),
        // Sample (B): Chinese (simplified).
        (
            "\u{4ED6}\u{4EEC}\u{4E3A}\u{4EC0}\u{4E48}\u{4E0D}\u{8BF4}\u{4E2D}\u{6587}",
            "ihqwcrb4cv8a8dqg056pqjye",
        ),
        // Sample (F): Hebrew.
        (
            "\u{05DC}\u{05DE}\u{05D4}\u{05D4}\u{05DD}\u{05E4}\u{05E9}\u{05D5}\u{05D8}\u{05DC}\u{05D0}\u{05DE}\u{05D3}\u{05D1}\u{05E8}\u{05D9}\u{05DD}\u{05E2}\u{05D1}\u{05E8}\u{05D9}\u{05EA}",
            "4dbcagdahymbxekheh6e0a7fei0b",
        ),
        // A hyphen among the basic code points stays in front of the delimiter.
        ("a-\u{FC}", "a--yka"),
    ] {
        assert_eq!(
            encode_label(label).as_deref(),
            Some(want),
            "label {label:?}"
        );
    }
}
