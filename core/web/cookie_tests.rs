//! Tests for `cookie.rs`: attributes, parsing `Cookie` headers, the name from the node key.

use super::*;
use crate::net::{NodeSecret, PeerId};
use axum::http::{header, HeaderMap, HeaderValue};

const ID: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ0123456789ABCDEFGHJK";

fn headers(lines: &[&str]) -> HeaderMap {
    let mut h = HeaderMap::new();
    for l in lines {
        h.append(header::COOKIE, HeaderValue::from_str(l).unwrap());
    }
    h
}

fn key(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

mod set_session {
    use super::*;

    #[test]
    fn has_exact_attributes() {
        let v = session_cookie_header("meshtown_ab", ID);
        let want = format!("meshtown_ab={ID}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800");
        assert_eq!(v.to_str().unwrap(), want);
    }
}

mod clear {
    use super::*;

    #[test]
    fn has_max_age_zero() {
        let v = expired_session_cookie_header("meshtown_ab");
        let want = "meshtown_ab=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0";
        assert_eq!(v.to_str().unwrap(), want);
    }
}

mod session_id {
    use super::*;

    #[test]
    fn single() {
        let h = headers(&[&format!("meshtown_ab={ID}")]);
        assert_eq!(session_id(&h, "meshtown_ab").as_deref(), Some(ID));
    }

    #[test]
    fn among_others() {
        let h = headers(&[&format!("a=1; meshtown_ab={ID}; b=2")]);
        assert_eq!(session_id(&h, "meshtown_ab").as_deref(), Some(ID));
    }

    #[test]
    fn second_cookie_header() {
        let h = headers(&["a=1", &format!("meshtown_ab={ID}")]);
        assert_eq!(session_id(&h, "meshtown_ab").as_deref(), Some(ID));
    }

    #[test]
    fn other_name_is_none() {
        let h = headers(&[&format!("meshtown_zz={ID}")]);
        assert_eq!(session_id(&h, "meshtown_ab"), None);
    }

    #[test]
    fn name_prefix_is_none() {
        let h = headers(&[&format!("meshtown_abc={ID}")]);
        assert_eq!(session_id(&h, "meshtown_ab"), None);
    }

    #[test]
    fn malformed_value_is_none() {
        let h = headers(&["meshtown_ab=short"]);
        assert_eq!(session_id(&h, "meshtown_ab"), None);
    }

    #[test]
    fn first_wins() {
        let other = ID.replace('0', "1");
        let h = headers(&[&format!("meshtown_ab={ID}; meshtown_ab={other}")]);
        assert_eq!(session_id(&h, "meshtown_ab").as_deref(), Some(ID));
    }

    #[test]
    fn no_header_is_none() {
        assert_eq!(session_id(&HeaderMap::new(), "meshtown_ab"), None);
    }
}

mod name_for {
    use super::*;

    #[test]
    fn prefix_and_ten_hex() {
        let n = session_cookie_name(&key(1));
        let tail = n.strip_prefix("meshtown_").unwrap();
        assert_eq!(tail.len(), 10);
        assert!(tail
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn same_key_same_name() {
        assert_eq!(session_cookie_name(&key(1)), session_cookie_name(&key(1)));
    }

    #[test]
    fn keys_differ_names_differ() {
        assert_ne!(session_cookie_name(&key(1)), session_cookie_name(&key(2)));
    }

    #[test]
    fn only_safe_chars() {
        let n = session_cookie_name(&key(7));
        assert!(n
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'));
    }
}

mod max_age {
    use super::*;

    #[test]
    fn equals_session_ttl() {
        assert_eq!(SESSION_MAX_AGE_SEC, 604_800);
    }
}

fn golden_id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

#[test]
fn golden_cookie_name_is_prefix_plus_first_ten_hex() {
    let a = golden_id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let b = golden_id_of("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
    // RFC 8032 §7.1 anchor
    assert_eq!(
        a.to_string(),
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    );
    assert_eq!(
        b.to_string(),
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"
    );
    assert_eq!(session_cookie_name(&a), "meshtown_d75a980182");
    assert_eq!(session_cookie_name(&b), "meshtown_3d4017c3e8");
}
