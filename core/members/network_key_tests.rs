//! Network key tests: encoding, subkeys, parsing of human input (`network_key.rs`).

use anyhow::Context;

use super::*;

#[test]
fn key_roundtrip() {
    let key = NetworkKey::generate().unwrap();
    let encoded = key.encode();
    let parsed = NetworkKey::parse(&encoded).unwrap();
    assert_eq!(key.0, parsed.0);
}

#[test]
fn rejects_wrong_prefix() {
    let err = NetworkKey::parse("wrong1-aaaaaaaa").unwrap_err();
    assert!(err.to_string().contains("mesh1-"));
}

#[test]
fn rejects_bad_characters() {
    let bad = format!("{}{}", KEY_PREFIX, "1".repeat(52));
    let err = NetworkKey::parse(&bad).unwrap_err();
    assert!(err.to_string().contains("base32"));
}

#[test]
fn rejects_wrong_length() {
    let bytes = [7u8; 31];
    let encoded = format!(
        "{}{}",
        KEY_PREFIX,
        BASE32_NOPAD.encode(&bytes).to_ascii_lowercase()
    );
    let err = NetworkKey::parse(&encoded).unwrap_err();
    assert!(err.to_string().contains("32 bytes"));
}

#[test]
fn accepts_uppercase_and_spaces() {
    let key = NetworkKey::generate().unwrap();
    let padded = format!("  {}  ", key.encode().to_uppercase());
    let parsed = NetworkKey::parse(&padded).unwrap();
    assert_eq!(key.0, parsed.0);
}

#[test]
fn subkeys_are_deterministic_and_distinct() {
    let key = NetworkKey::generate().unwrap();
    let s1 = key.subkeys();
    let s2 = key.subkeys();
    let vecs: Vec<Vec<u8>> = vec![
        s1.dht_signing_seed.to_vec(),
        s1.dht_salt.to_vec(),
        s1.roster_key.to_vec(),
        s1.handshake_key.to_vec(),
        s1.gossip_topic.to_vec(),
        s1.docs_namespace_seed.to_vec(),
        s1.network_id.to_vec(),
    ];
    for i in 0..vecs.len() {
        for j in (i + 1)..vecs.len() {
            assert_ne!(vecs[i], vecs[j], "subkeys {i} and {j} matched");
        }
    }
    assert_eq!(s1.dht_signing_seed, s2.dht_signing_seed);
    assert_eq!(s1.network_id, s2.network_id);

    let other_key = NetworkKey::generate().unwrap();
    let s3 = other_key.subkeys();
    assert_ne!(s1.dht_signing_seed, s3.dht_signing_seed);
}

#[test]
fn parse_does_not_panic_on_multibyte() {
    assert!(NetworkKey::parse("abcdeøøøø").is_err());
    assert!(!NetworkKey::looks_like_encoded_key("abcdeøøøø"));
}

#[test]
fn user_input_routes_mesh1_prefix_to_parse() {
    let key = NetworkKey::generate().unwrap();
    let padded = format!("  {}  ", key.encode().to_uppercase());
    assert_eq!(NetworkKey::from_user_input(&padded).unwrap().0, key.0);
    let err =
        NetworkKey::from_user_input("Mesh1-this is not a key, but a long phrase").unwrap_err();
    assert!(err.to_string().contains("base32"), "{err}");
    let err = NetworkKey::from_user_input("   ").unwrap_err();
    assert!(err.to_string().contains("meshtown keygen"), "{err}");
}

/// Protocol reference: recompute with `printf 'meshtown network key tail v1\n%s' <tail> | shasum -a 256`;
/// `KAT_ENCODED` is `mesh1-` plus the base32 (lowercase, no padding) of the hash.
const KAT_TAIL: &str = "finch-unwary-k7m2q9x4rb8vhn3tz01g5";
const KAT_HEX: &str = "bbabafe34d70062c4b1ee7cfe9232ce2a05b254b9d8025afe2b4868da2e6a1c0";
const KAT_ENCODED: &str = "mesh1-xov27y2noadcysy647h6sizm4kqfwjkltwacll7cwsdi3ixguhaa";

#[test]
fn normalize_dashed_rules() {
    assert_eq!(normalize_dashed("  Our Gaming_Room  "), "our-gaming-room");
    assert_eq!(normalize_dashed("our---gaming"), "our-gaming");
    assert_eq!(normalize_dashed("--our--"), "our");
    assert_eq!(normalize_dashed("ＯＵＲ ROOM"), "our-room");
    assert_eq!(normalize_dashed("a\u{00A0}b"), "a-b");
    assert_eq!(normalize_dashed("net!?"), "net");
    assert_eq!(normalize_dashed("東京 ネット №1"), "東京-ネット-no1");
}

#[test]
fn normalize_label_keeps_any_alphabet() {
    assert_eq!(normalize_label("Ünser Bäum").unwrap(), "ünser-bäum");
    assert!(!normalize_label("私 の ネット").unwrap().is_empty());
    assert_eq!(normalize_label("ROOM 42").unwrap(), "room-42");
}

#[test]
fn normalize_label_rejects_too_long() {
    let too_long: String = "a".repeat(65);
    let err = normalize_label(&too_long).unwrap_err().to_string();
    assert!(err.contains("64"), "{err}");
    let ok: String = "a".repeat(64);
    assert!(normalize_label(&ok).is_ok());
}

#[test]
fn normalize_label_rejects_empty_after_normalization() {
    let err = normalize_label("!!! ???").unwrap_err().to_string();
    assert!(err.contains("letters"), "{err}");
}

#[test]
fn derive_tail_is_deterministic_and_input_sensitive() {
    let a1 = derive_tail_bytes(KAT_TAIL);
    let a2 = derive_tail_bytes(KAT_TAIL);
    let b = derive_tail_bytes("finch-unwary-k7m2q9x4rb8vhn3tz01g6");
    assert_eq!(a1, a2);
    assert_ne!(a1, b);
    assert_ne!(derive_tail_bytes(""), derive_tail_bytes("a"));
}

#[test]
fn splits_on_first_double_dash() {
    let err = parse_user_input(&format!("a--b--{KAT_TAIL}"))
        .unwrap_err()
        .to_string();
    assert!(err.contains('4') && err.contains('3'), "{err}"); // 4 parts: "b" landed in the random one
    assert_eq!(
        parse_user_input(&format!("our--{KAT_TAIL}"))
            .unwrap()
            .label
            .as_deref(),
        Some("our")
    );
}

#[test]
fn label_does_not_change_key() {
    let a = parse_user_input(&format!("our-gaming-room--{KAT_TAIL}")).unwrap();
    let b = parse_user_input(&format!("other label--{KAT_TAIL}")).unwrap();
    let c = parse_user_input(KAT_TAIL).unwrap();
    assert_eq!(a.key.0, b.key.0);
    assert_eq!(a.key.0, c.key.0);
    assert_eq!(
        a.key.subkeys().network_id_hex(),
        b.key.subkeys().network_id_hex()
    );
    assert_eq!(
        a.key.subkeys().network_id_hex(),
        c.key.subkeys().network_id_hex()
    );
    assert_ne!(a.label, b.label);
}

#[test]
fn word_error_names_only_the_bad_word() {
    let bad = KAT_TAIL.replace("unwary", "zzqq");
    let err = key_from_tail(&bad).unwrap_err().to_string();
    assert!(err.contains("zzqq"), "{err}");
    assert!(!err.contains("finch"), "{err}");
    assert!(!err.contains("k7m2q9x4rb8vhn3tz01g5"), "{err}");
}

#[test]
fn bad_chunk_is_not_echoed() {
    let bad = format!("{KAT_TAIL}x");
    let err = parse_user_input(&bad).unwrap_err().to_string();
    assert!(err.contains("21"), "{err}");
    assert!(err.contains("22"), "{err}");
    assert!(!err.contains("k7m2q9x4rb8vhn3tz01g5x"), "{err}");
}

#[test]
fn label_without_tail_is_rejected() {
    let err = parse_user_input("our gaming room").unwrap_err().to_string();
    assert!(err.contains("only the label"), "{err}");
    assert!(err.contains("our-gaming-room"), "{err}");
    assert!(err.contains("keygen"), "{err}");
    assert!(is_tail_word("gaming") && !is_tail_word("our") && !is_tail_word("room"));
}

#[test]
fn tail_without_label_is_accepted() {
    let parsed = parse_user_input(KAT_TAIL).unwrap();
    assert!(parsed.label.is_none());
    assert!(parsed.from_words);
}

#[test]
fn tail_with_wrong_part_count_rejected() {
    let err = key_from_tail("finch-unwary").unwrap_err().to_string();
    assert!(err.contains('3'), "{err}");
    let err = parse_user_input("our--").unwrap_err().to_string();
    assert!(err.contains("after \"--\""), "{err}");
}

#[test]
fn key_from_tail_known_answer() {
    let k = key_from_tail(KAT_TAIL).unwrap();
    assert_eq!(
        HEXLOWER.encode(&k.0),
        KAT_HEX,
        "the reference changed — this breaks all networks"
    );
    assert_eq!(k.encode(), KAT_ENCODED);
}

#[test]
fn key_from_tail_normalizes_input() {
    let a = key_from_tail(KAT_TAIL).unwrap();
    let b = key_from_tail(&format!("{KAT_TAIL}-")).unwrap();
    let c = key_from_tail("  FINCH unwary_K7M2Q9X4RB8VHN3TZ01G5 ").unwrap();
    let d = key_from_tail("finch-unwary-k7m2q9x4rb8vhn3tzOIg5").unwrap();
    assert_eq!(a.0, b.0);
    assert_eq!(a.0, c.0);
    assert_eq!(a.0, d.0);
}

#[test]
fn generated_key_roundtrips() {
    for _ in 0..50 {
        let tail = crate::members::words::generate_tail().unwrap();
        let s = format_key_string(Some("our network"), &tail);
        let parsed = parse_user_input(&s).unwrap();
        assert_eq!(parsed.label.as_deref(), Some("our-network"));
        assert_eq!(parsed.key.0, key_from_tail(&tail).unwrap().0);
    }
}

#[test]
fn bad_word_downcasts_from_question_mark_and_context() {
    fn via_question_mark(tail: &str) -> Result<NetworkKey> {
        let key = key_from_tail(tail)?;
        Ok(key)
    }
    let bad = KAT_TAIL.replace("unwary", "zzqq");
    let err = via_question_mark(&bad).unwrap_err();
    let bad_word = err.downcast_ref::<BadWord>().expect("BadWord through ?");
    assert_eq!(bad_word.word, "zzqq");
    assert_eq!(bad_word.to_string(), bad_word.message);

    let err = key_from_tail(&bad)
        .context("context around BadWord")
        .unwrap_err();
    let bad_word = err
        .downcast_ref::<BadWord>()
        .expect("BadWord through .context()");
    assert_eq!(bad_word.word, "zzqq");
    assert!(format!("{err:#}").contains("context around BadWord"));
}

#[test]
fn key_with_label_round_trips() {
    let key = NetworkKey::generate().unwrap();
    let with_label = key_with_label(Some("our network"), &key);
    let parsed = parse_user_input(&with_label).unwrap();
    assert_eq!(parsed.key.encode(), key.encode());
    assert_eq!(parsed.label.as_deref(), Some("our-network"));
    assert!(!parsed.from_words);

    let without_label = key_with_label(None, &key);
    assert_eq!(without_label, key.encode());
    let parsed = parse_user_input(&without_label).unwrap();
    assert_eq!(parsed.key.encode(), key.encode());
    assert_eq!(parsed.label, None);
    assert!(!parsed.from_words);
}

#[test]
fn label_with_encoded_tail_is_parsed_as_key_not_words() {
    let key = NetworkKey::generate().unwrap();
    let input = format!("label--{}", key.encode().to_uppercase());
    let parsed = parse_user_input(&input).unwrap();
    assert_eq!(parsed.key.encode(), key.encode());
    assert_eq!(parsed.label.as_deref(), Some("label"));
    assert!(!parsed.from_words);
}

#[test]
fn label_looking_like_mesh1_is_not_parsed_as_key() {
    let parsed = parse_user_input(&format!("mesh1-test--{KAT_TAIL}")).unwrap();
    assert_eq!(parsed.label.as_deref(), Some("mesh1-test"));
    assert_eq!(parsed.key.0, key_from_tail(KAT_TAIL).unwrap().0);
    let err = parse_user_input("mesh1-11111111").unwrap_err().to_string();
    assert!(err.contains("base32"), "{err}");
}

#[test]
fn protocol_strings_are_meshtown_v1() {
    let all = [
        CTX_DHT_SIGNING,
        CTX_DHT_SALT,
        CTX_ROSTER,
        CTX_HANDSHAKE,
        CTX_GOSSIP_TOPIC,
        CTX_DOCS_NAMESPACE,
        CTX_NETWORK_ID,
        CTX_TAIL_SALT,
    ];
    for ctx in all {
        assert!(ctx.starts_with("meshtown "), "{ctx}");
        assert!(ctx.ends_with(" v1"), "{ctx}");
        assert!(
            !ctx.as_bytes()
                .windows(4)
                .any(|w| w.iter().all(u8::is_ascii_digit)),
            "{ctx}"
        );
    }
    let set: std::collections::HashSet<&str> = all.into_iter().collect();
    assert_eq!(set.len(), all.len());
}
