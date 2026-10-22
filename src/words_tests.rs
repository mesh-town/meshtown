//! Tests of the word list and the random part of a key (`words.rs`).

use super::*;
use data_encoding::HEXLOWER;

#[test]
fn wordlist_has_7776_distinct_words() {
    let sum = ring::digest::digest(&ring::digest::SHA256, EFF_LARGE_WORDLIST.as_bytes());
    assert_eq!(HEXLOWER.encode(sum.as_ref()), WORDLIST_SHA256);
    assert_eq!(EFF_LARGE_WORDLIST.lines().count(), WORDLIST_LEN);
    assert_eq!(wordlist().len(), WORDLIST_LEN);
    assert_eq!(tail_pool().len(), TAIL_POOL_LEN);
    assert_eq!(WORDLIST_LEN - TAIL_POOL_LEN, 4);
    for w in wordlist() {
        assert!(
            !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "word {w:?}"
        );
    }
}

#[test]
fn tail_pool_excludes_hyphenated_words() {
    assert_eq!(tail_pool().len(), 7_772);
    for bad in ["drop-down", "felt-tip", "t-shirt", "yo-yo"] {
        assert!(!is_tail_word(bad), "{bad}");
    }
    for w in tail_pool_vec() {
        assert!(!w.contains('-'), "{w}");
    }
    assert!(is_tail_word("finch"));
}

#[test]
fn unbiased_limit_is_largest_multiple_of_pool() {
    assert_eq!(UNBIASED_LIMIT as usize % TAIL_POOL_LEN, 0);
    assert!(UNBIASED_LIMIT as usize + TAIL_POOL_LEN > 65_536);
    assert_eq!(65_536 - UNBIASED_LIMIT as usize, 65_536 % TAIL_POOL_LEN);
}

#[test]
fn generated_tail_has_two_pool_words_and_a_chunk() {
    for _ in 0..50 {
        let t = generate_tail().unwrap();
        let parts: Vec<&str> = t.split('-').collect();
        assert_eq!(parts.len(), TAIL_PARTS, "{t}");
        assert_ne!(parts[0], parts[1], "{t}");
        assert!(is_tail_word(parts[0]), "{t}");
        assert!(is_tail_word(parts[1]), "{t}");
        assert!(is_tail_chunk(parts[TAIL_WORDS]), "{t}");
        assert_eq!(crate::network_key::normalize_dashed(&t), t);
    }
}

#[test]
fn generated_tails_differ() {
    let mut set: HashSet<String> = HashSet::new();
    for _ in 0..20 {
        set.insert(generate_tail().unwrap());
    }
    assert_eq!(set.len(), 20);
}

#[test]
fn random_word_index_stays_in_pool() {
    let mut seen: HashSet<usize> = HashSet::new();
    for _ in 0..20_000 {
        let i = random_word_index().unwrap();
        assert!(i < TAIL_POOL_LEN);
        seen.insert(i);
    }
    assert!(seen.len() >= 5_000, "{}", seen.len());
}

#[test]
fn chunk_alphabet_is_crockford_and_covered() {
    assert_eq!(TAIL_ALPHABET.chars().count(), 32);
    let chars: HashSet<char> = TAIL_ALPHABET.chars().collect();
    assert_eq!(chars.len(), 32);
    for bad in ['i', 'l', 'o', 'u'] {
        assert!(!chars.contains(&bad), "{bad}");
    }
    for d in '0'..='9' {
        assert!(chars.contains(&d), "{d}");
    }
    assert_eq!(256 % 32, 0);
    assert!(is_tail_chunk(&"a".repeat(21)));
    assert!(!is_tail_chunk(&"a".repeat(20)));
    assert!(!is_tail_chunk(&"a".repeat(22)));
    assert!(!is_tail_chunk(&"u".repeat(21)));
    let mut seen_chars: HashSet<char> = HashSet::new();
    for _ in 0..200 {
        let t = generate_tail().unwrap();
        let chunk = t.rsplit('-').next().unwrap();
        seen_chars.extend(chunk.chars());
    }
    assert!(seen_chars.len() >= 30, "{}", seen_chars.len());
}

#[test]
fn canonical_chunk_maps_confusable_letters() {
    assert_eq!(canonical_chunk("o0i1l"), "00111");
    assert_eq!(canonical_chunk("abc123"), "abc123");
    let mapped = canonical_chunk("o0i1l");
    assert_eq!(canonical_chunk(&mapped), mapped);
    let x = "oil23456789abcdefghjk";
    assert_eq!(x.chars().count(), TAIL_CHUNK_CHARS);
    assert!(is_tail_chunk(&canonical_chunk(x)));
}

#[test]
fn default_name_is_two_pool_words() {
    let id = [7u8; 32];
    let name = default_node_name(&id);

    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 2, "{name}");
    assert!(is_tail_word(parts[0]), "{name}");
    assert!(is_tail_word(parts[1]), "{name}");
    assert_ne!(parts[0], parts[1], "{name}");
    assert_eq!(trim_and_validate_node_name(&name), Ok(name.as_str()));
}

/// The default name for `id = [0x11; 32]` is fixed (recompute: `b3sum --derive-key 'meshtown node name v1'` over the id).
#[test]
fn default_name_golden_for_id_11() {
    let id = [0x11u8; 32];
    let name = default_node_name(&id);
    assert_eq!(name, "motivator-deceased");
}

#[test]
fn default_name_is_stable_for_id() {
    let id = [5u8; 32];
    assert_eq!(
        default_node_name(&id),
        default_node_name(&id),
        "a repeated call gave a different name"
    );
}

#[test]
fn default_name_differs_by_id() {
    let a = default_node_name(&[1u8; 32]);
    let b = default_node_name(&[2u8; 32]);
    assert_ne!(a, b);
}

#[test]
fn default_name_golden() {
    let name = default_node_name(&[0u8; 32]);
    assert_eq!(name, "calzone-tinsel");
}

#[test]
fn random_name_is_two_distinct_pool_words() {
    let name = random_node_name().expect("random_node_name");
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 2, "{name}");
    assert!(is_tail_word(parts[0]), "{name}");
    assert!(is_tail_word(parts[1]), "{name}");
    assert_ne!(parts[0], parts[1], "{name}");
    assert_eq!(trim_and_validate_node_name(&name), Ok(name.as_str()));
}

#[test]
fn random_names_vary() {
    let mut set: HashSet<String> = HashSet::new();
    for _ in 0..20 {
        set.insert(random_node_name().expect("random_node_name"));
    }
    assert!(set.len() >= 2, "names do not differ: {set:?}");
}

#[test]
fn check_node_name_accepts_and_trims() {
    assert_eq!(trim_and_validate_node_name("  Kitchen  "), Ok("Kitchen"));

    let sixty_four_two_byte = "é".repeat(64);
    assert_eq!(sixty_four_two_byte.len(), 128);
    assert_eq!(
        trim_and_validate_node_name(&sixty_four_two_byte),
        Ok(sixty_four_two_byte.as_str())
    );

    let sixty_four_four_byte = "𝄞".repeat(64);
    assert_eq!(sixty_four_four_byte.len(), MAX_NAME_BYTES);
    assert_eq!(
        trim_and_validate_node_name(&sixty_four_four_byte),
        Ok(sixty_four_four_byte.as_str())
    );
}

#[test]
fn check_node_name_rejects() {
    assert_eq!(trim_and_validate_node_name(""), Err("name is empty"));
    assert_eq!(trim_and_validate_node_name("   "), Err("name is empty"));

    let sixty_five_two_byte = "é".repeat(65);
    assert_eq!(
        trim_and_validate_node_name(&sixty_five_two_byte),
        Err("name is longer than 64 characters")
    );

    for bad in ["a\nb", "a\tb", "a\u{2028}b", "a\u{7f}b"] {
        assert_eq!(
            trim_and_validate_node_name(bad),
            Err("name contains a control character or a line break"),
            "{bad:?}"
        );
    }
}
