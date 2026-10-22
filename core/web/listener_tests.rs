//! Tests for `listener.rs`: the startup announce text.

use super::*;

fn url(n: u8) -> String {
    format!("http://127.0.0.{n}:7070/")
}

mod announce_text {
    use super::*;

    #[test]
    fn without_password_says_ten_minutes_once() {
        let text = announce_text(&[url(1)], false);
        assert!(text.contains("10 minutes"), "{text}");
        assert!(text.contains("single use"), "{text}");
        assert!(text.contains(&url(1)), "{text}");
    }

    #[test]
    fn with_password_says_by_password() {
        let text = announce_text(&[url(1)], true);
        assert!(text.contains("password sign-in"), "{text}");
        assert!(!text.contains("10 minutes"), "{text}");
    }

    #[test]
    fn two_urls_joined() {
        let text = announce_text(&[url(1), url(2)], true);
        assert!(text.contains(&format!("{} or {}", url(1), url(2))), "{text}");
    }
}
