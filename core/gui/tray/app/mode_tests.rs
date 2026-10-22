use super::*;

#[test]
fn headless_from_display_true_when_both_empty() {
    assert!(headless_from_display(None, None));
    assert!(headless_from_display(Some(""), Some("")));
}

#[test]
fn headless_from_display_false_when_either_set() {
    assert!(!headless_from_display(Some(":0"), None));
    assert!(!headless_from_display(None, Some("wayland-0")));
}

#[test]
fn graphic_access_from_bits_true_when_flag_set() {
    assert!(graphic_access_from_bits(0, 0x0010));
}

#[test]
fn graphic_access_from_bits_false_on_error_status() {
    assert!(!graphic_access_from_bits(1, 0x0010));
}

#[test]
fn is_headless_session_zero_is_headless() {
    assert!(is_headless_session(0));
    assert!(!is_headless_session(1));
}
