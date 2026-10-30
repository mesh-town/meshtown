//! Network key tests: subkeys, derivation from the random part, parsing of human input (`network_key.rs`).

use anyhow::Context;

use super::*;

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

/// Protocol reference: recompute with `printf 'meshtown network key tail v1\n%s' <tail> | shasum -a 256`.
const KAT_TAIL: &str = "finch-unwary-k7m2q9x4rb8vhn3tz01g5";
const KAT_HEX: &str = "bbabafe34d70062c4b1ee7cfe9232ce2a05b254b9d8025afe2b4868da2e6a1c0";

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

/// A creator for the Managed forms of the test keys.
fn root() -> crate::transport::net::PeerId {
    test_net("x").root
}

#[test]
fn splits_on_first_double_dash() {
    let text = format!("a--b--{}", format_managed_key(None, KAT_TAIL, &root()));
    let err = parse_user_input(&text).unwrap_err().to_string();
    assert!(err.contains('5') && err.contains('3'), "{err}"); // 5 parts: "b" landed in the random one
    assert_eq!(
        parse_user_input(&format_managed_key(Some("our"), KAT_TAIL, &root()))
            .unwrap()
            .label
            .as_deref(),
        Some("our")
    );
}

#[test]
fn label_does_not_change_key() {
    let root = root();
    let text = |label| format_managed_key(label, KAT_TAIL, &root);
    let a = parse_user_input(&text(Some("our-gaming-room"))).unwrap();
    let b = parse_user_input(&text(Some("other label"))).unwrap();
    let c = parse_user_input(&text(None)).unwrap();
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
    let bad = format_managed_key(None, &format!("{KAT_TAIL}x"), &root());
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
    assert!(err.contains("network create --key-only"), "{err}");
    assert!(is_tail_word("gaming") && !is_tail_word("our") && !is_tail_word("room"));
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
        let root = crate::transport::net::NodeSecret::generate().peer_id();
        let s = format_managed_key(Some("our network"), &tail, &root);
        let parsed = parse_user_input(&s).unwrap();
        assert_eq!(parsed.label.as_deref(), Some("our-network"));
        assert_eq!(parsed.root, root);
        assert_eq!(parsed.base, key_from_tail(&tail).unwrap().0);
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
fn managed_key_with_label_round_trips() {
    let net = test_net("our network");
    let parsed = parse_user_input(&net.text).unwrap();
    assert_eq!(parsed.key.0, net.key.0);
    assert_eq!(parsed.label.as_deref(), Some("our-network"));
    assert!(!parsed.from_words);

    let without_label = managed_key_with_label(None, &net.base, &net.root);
    let parsed = parse_user_input(&without_label).unwrap();
    assert_eq!(parsed.key.0, net.key.0);
    assert_eq!(parsed.label, None);
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
