use super::*;
use crate::network_key::NetworkKey;

#[test]
fn record_key_golden_for_network_key_44() {
    let sub = NetworkKey::from_bytes([0x44; 32]).subkeys();
    let keys = RosterKeys::from_subkeys(&sub);
    let hex: String = keys.record_key.seed().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, "0acb831ee7352b1e30e07dd8077bc9fc5b38f249174015bf62daa5e490b442a6");
    assert_eq!(keys.seal_key, sub.roster_key);
}

#[test]
fn record_key_differs_from_mainline_seed_and_roster_key() {
    let sub = NetworkKey::from_bytes([0x44; 32]).subkeys();
    let keys = RosterKeys::from_subkeys(&sub);
    assert_ne!(keys.record_key.seed(), sub.dht_signing_seed);
    assert_ne!(keys.record_key.seed(), sub.roster_key);
}

#[test]
fn protocol_strings_are_meshtown_v1() {
    assert_eq!(CTX_RECORD, "meshtown hyperdht roster signing v1");
}
