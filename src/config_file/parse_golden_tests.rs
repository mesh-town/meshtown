use super::*;
use crate::config_file::test_support::secret_of;
use crate::net::{NodeSecret, PeerId};

fn id_of(seed_hex: &str) -> PeerId {
    let seed: [u8; 32] = data_encoding::HEXLOWER
        .decode(seed_hex.as_bytes())
        .expect("seed — hex")
        .try_into()
        .expect("seed is 32 bytes");
    NodeSecret::from_bytes(&seed).peer_id()
}

fn file_with_exits(exits: &str) -> String {
    format!(
        r#"{{"rev":1,"identity":{{"secret":"BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="}},"config":{{"version":1,"route":{{"exits":{exits}}}}}}}"#
    )
}

#[test]
fn golden_parse_reads_seed_and_exits_from_literals() {
    const KEY1: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const KEY2: &str = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c";
    const KEY2_BASE32: &str = "hvabpq7iioevvevxbktu2g36xsojqlgpf3cjndgazvk7ckxumyga";
    let a = id_of("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
    let b = id_of("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
    // anchor: RFC 8032 §7.1
    assert_eq!(a.to_string(), KEY1);
    assert_eq!(b.to_string(), KEY2);

    let p =
        parse_text(&file_with_exits(&format!(r#"["{KEY1}","{KEY2_BASE32}"]"#))).expect("parse");
    assert_eq!(secret_of(&p), [7u8; 32]);
    assert_eq!(p.field_errors, vec![]);
    assert_eq!(p.settings.net.route_exits, vec![a, b]);

    // uppercase hex is not accepted — the default stays
    let upper = KEY1.to_uppercase();
    let p = parse_text(&file_with_exits(&format!(r#"["{upper}"]"#))).expect("parse");
    assert!(p.settings.net.route_exits.is_empty(), "uppercase hex was accepted");
}
