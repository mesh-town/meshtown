//! Tests of the check record key and of parsing the domain out of it (`keys.rs`).

use super::*;

#[test]
fn check_key_has_terminator() {
    assert_eq!(check_key("example.com"), b"chk/example.com/".to_vec());
}

#[test]
fn domain_from_check_key_rejects_foreign() {
    assert_eq!(
        domain_from_check_key(b"chk/example.com/"),
        Some("example.com".to_string())
    );
    assert_eq!(domain_from_check_key(b"list/example.com/"), None);
    assert_eq!(domain_from_check_key(b"chk/example.com"), None);
    assert_eq!(domain_from_check_key(b"chk/Example.COM/"), None);
}

fn view(domain: &str, author_seed: u8) -> CheckView {
    CheckView {
        domain: domain.to_string(),
        author: crate::transport::net::NodeSecret::from_bytes(&[author_seed; 32]).peer_id(),
        entry_ts_micros: 0,
        record: None,
    }
}

#[test]
fn only_listed_keeps_listed_domain() {
    let views = vec![view("a.com", 1), view("a.com", 2), view("z.com", 2)];
    let out = only_listed(views, &["a.com".to_string()]);
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|v| v.domain == "a.com"));
}

#[test]
fn only_listed_empty_list_is_empty() {
    let views = vec![view("a.com", 1), view("a.com", 2), view("z.com", 2)];
    assert!(only_listed(views, &[]).is_empty());
}
