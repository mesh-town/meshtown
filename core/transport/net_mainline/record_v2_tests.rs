//! Tests of the roster v2 record (keys only, for the `hyperdht` source): encoding, `merge_keys`, `open_as`.

use super::*;

const T: u64 = 1_760_000_000;

fn key_entry(n: u8, seen: u64) -> RosterEntry {
    RosterEntry {
        id: [n; 32],
        relay: None,
        addrs: vec![],
        seen_unix: seen,
    }
}

fn key_roster(entries: Vec<RosterEntry>) -> Roster {
    Roster {
        version: RECORD_VERSION_KEYS,
        entries,
    }
}

#[test]
fn key_roster_plaintext_golden() {
    let bytes = postcard::to_allocvec(&key_roster(vec![key_entry(1, T)])).unwrap();
    let mut want = vec![0x02, 0x01];
    want.extend_from_slice(&[0x01; 32]);
    want.extend_from_slice(&[0x00, 0x00, 0x80, 0xf0, 0x9d, 0xc7, 0x06]);
    assert_eq!(bytes.len(), 41);
    assert_eq!(bytes, want);
}

#[test]
fn sealed_key_roster_golden() {
    const SEALED: &str = "2222222222222222222222228774d41f3c91ac76dc63f0fdfa03cb6e26fc40995ad3d3f77f5c2ee0b495ab564b28a278545cd60280b729db30d88b345a5906f61d1ed7a90e";
    let roster = key_roster(vec![key_entry(1, T)]);
    let sealed = seal(&[0x11; 32], &roster, [0x22; 12]).unwrap();
    assert_eq!(data_encoding::HEXLOWER.encode(&sealed), SEALED);
    let literal = data_encoding::HEXLOWER.decode(SEALED.as_bytes()).unwrap();
    assert_eq!(
        open_as(&[0x11; 32], &literal, RECORD_VERSION_KEYS).unwrap(),
        roster
    );
}

#[test]
fn sixteen_key_entries_fit_654_bytes() {
    let entries = (1..=16u8).map(|n| key_entry(n, T)).collect();
    let sealed = seal(&[0x11; 32], &key_roster(entries), [0x22; 12]).unwrap();
    assert_eq!(sealed.len(), 654);
    assert!(sealed.len() <= 990);
}

#[test]
fn merge_keys_strips_addresses_and_sets_v2() {
    let mut a = key_entry(2, T);
    a.relay = Some("https://relay.example/".to_string());
    a.addrs = vec!["203.0.113.5:4000".parse().unwrap()];
    let prev = Roster {
        version: 1,
        entries: vec![a],
    };
    let got = merge_keys(Some(prev), [9; 32], T);
    assert_eq!(got.version, 2);
    assert_eq!(got.entries.len(), 2);
    assert!(got
        .entries
        .iter()
        .all(|e| e.relay.is_none() && e.addrs.is_empty()));
}

#[test]
fn merge_keys_caps_at_sixteen_me_first() {
    let prev = key_roster((1..=20u8).map(|n| key_entry(n, T)).collect());
    let got = merge_keys(Some(prev), [99; 32], T);
    assert_eq!(got.entries.len(), 16);
    assert_eq!(got.entries[0].id, [99; 32]);
}

#[test]
fn merge_keys_drops_stale_and_self_duplicate() {
    let prev = key_roster(vec![
        key_entry(5, T - ENTRY_TTL_SECS - 1),
        key_entry(7, T - 10),
        key_entry(8, T - 20),
    ]);
    let got = merge_keys(Some(prev), [7; 32], T);
    let ids: Vec<u8> = got.entries.iter().map(|e| e.id[0]).collect();
    assert_eq!(ids, vec![7, 8]);
    assert_eq!(got.entries[0].seen_unix, T);
}

#[test]
fn open_as_rejects_other_version() {
    let key = [0x11; 32];
    let v1 = Roster {
        version: 1,
        entries: vec![key_entry(1, T)],
    };
    let sealed_v1 = seal(&key, &v1, [0x22; 12]).unwrap();
    let err = open_as(&key, &sealed_v1, RECORD_VERSION_KEYS).unwrap_err();
    assert!(err.to_string().contains("unknown record version"), "{err}");
    let sealed_v2 = seal(&key, &key_roster(vec![key_entry(1, T)]), [0x22; 12]).unwrap();
    let err = open_as(&key, &sealed_v2, 1).unwrap_err();
    assert!(err.to_string().contains("unknown record version"), "{err}");
}
