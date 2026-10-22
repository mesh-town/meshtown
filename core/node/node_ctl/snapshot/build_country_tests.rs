//! Test of the `country` field in the snapshot: a member's country
//! is taken from `Inputs.countries` without an "online only" condition — it is shown for an offline member
//! the same way as for an online one. `PeerInfo`/`ChosenExit` serialize a missing country as `null`,
//! without `skip_serializing_if`. It does not check reading from the network document (the stub
//! `HashMap::new()` in `collect.rs` is read by group G2). The helpers `base`/`pair` come from
//! `super::tests` (`pub(super)`), as in `build_exit_tests.rs`.

use std::time::Instant;

use super::tests::{base, pair};
use super::*;
use crate::telemetry::PeerLive;

fn offline_peer() -> PeerLive {
    PeerLive {
        online: false,
        path: None,
        rtt_ms: None,
        last_seen: Instant::now(),
    }
}

#[test]
fn self_row_takes_country_from_map_or_none_without_record() {
    let my_id = pair(1);
    let mut inputs = base(my_id);
    inputs.countries = [(my_id, "RU".to_string())].into_iter().collect();

    let peers = build_peers(&inputs);
    assert_eq!(
        peers[0].country.as_deref(),
        Some("RU"),
        "own row takes the country from countries[i.id]"
    );

    inputs.countries.clear();
    let peers = build_peers(&inputs);
    assert_eq!(peers[0].country, None, "no own record — None");
}

#[test]
fn other_peer_country_shown_online_and_offline_alike() {
    let my_id = pair(1);
    let online_with_record = pair(2);
    let offline_with_record = pair(3);
    let online_without_record = pair(4);

    let mut inputs = base(my_id);
    inputs.members = [online_with_record, online_without_record]
        .into_iter()
        .collect();
    inputs.telemetry.peers.insert(offline_with_record, offline_peer());
    inputs.countries = [
        (online_with_record, "RU".to_string()),
        (offline_with_record, "DE".to_string()),
    ]
    .into_iter()
    .collect();

    let peers = build_peers(&inputs);
    let country_of = |id: PeerId| {
        peers
            .iter()
            .find(|p| p.id == output::short_peer_id(&id))
            .unwrap_or_else(|| panic!("no row for {id}"))
            .country
            .clone()
    };

    assert_eq!(
        country_of(online_with_record),
        Some("RU".to_string()),
        "another member online with a record — Some"
    );
    assert_eq!(
        country_of(offline_with_record),
        Some("DE".to_string()),
        "another member offline with a record — also Some"
    );
    assert_eq!(
        country_of(online_without_record),
        None,
        "online without a record in countries — None"
    );
}

#[test]
fn chosen_exit_country_from_map_or_none_without_record() {
    let my_id = pair(1);
    let exit_with_record = pair(2);
    let exit_without_record = pair(3);

    let mut inputs = base(my_id);
    inputs.members = [exit_with_record, exit_without_record]
        .into_iter()
        .collect();
    inputs.exits = [(exit_with_record, true), (exit_without_record, true)]
        .into_iter()
        .collect();
    inputs.countries = [(exit_with_record, "RU".to_string())].into_iter().collect();
    inputs.chosen_exits = vec![exit_with_record, exit_without_record];

    let collected = build(inputs);
    let country_of = |id: PeerId| {
        collected
            .snap
            .route
            .exits
            .iter()
            .find(|e| e.id_full == id.to_string())
            .unwrap_or_else(|| panic!("no ChosenExit for {id}"))
            .country
            .clone()
    };

    assert_eq!(
        country_of(exit_with_record),
        Some("RU".to_string()),
        "the chosen exit takes the country from countries"
    );
    assert_eq!(
        country_of(exit_without_record),
        None,
        "the chosen exit without a record — None"
    );
}

#[test]
fn peer_info_json_serializes_country_as_null_or_string() {
    let none_json = serde_json::to_value(PeerInfo {
        country: None,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        none_json.get("country"),
        Some(&serde_json::Value::Null),
        "no country — the field exists and equals null (without skip_serializing_if)"
    );

    let some_json = serde_json::to_value(PeerInfo {
        country: Some("RU".to_string()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        some_json.get("country"),
        Some(&serde_json::Value::String("RU".to_string())),
        "a known country — a code string"
    );
}
