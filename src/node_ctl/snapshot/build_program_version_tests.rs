//! Tests of the `version` field (`Inputs.program_versions` → `PeerInfo.program_version`) in the snapshot:
//! a member's version is taken from the map
//! `program_versions[id]` for both the own row and another member's, without an "online only" condition; no
//! record — `None`. In JSON the field is called `version` and without a version equals `null`; the chosen
//! exit (`route.exits[]`) has no version field. It does not check reading the network document or inserting the own
//! version — that is `collect_program_version_tests.rs`. The helpers `base`/`pair` come from `super::tests`.

use std::time::Instant;

use super::tests::{base, pair};
use super::*;
use crate::telemetry::PeerLive;

mod self_row {
    use super::*;

    #[test]
    fn self_row_takes_version_from_map() {
        // given: the version map has an entry for the node itself (different from the build version)
        let my_id = pair(1);
        let mut inputs = base(my_id);
        inputs.program_versions = versions([(my_id, "7.7.7")]);
        // when: the member list is built
        let peers = build_peers(&inputs);
        // then: the own row shows the version from the map
        assert_eq!(
            version_of(&peers, my_id),
            Some("7.7.7".to_string()),
            "own row takes the version from program_versions[i.id]"
        );
    }

    #[test]
    fn self_row_without_record_has_no_version() {
        // given: the version map has no entry for the node itself
        let my_id = pair(1);
        let inputs = base(my_id);
        // when: the member list is built
        let peers = build_peers(&inputs);
        // then: the own row has no version: `collect` supplies it, not `build`
        assert_eq!(version_of(&peers, my_id), None, "no record — None");
    }
}

mod other_row {
    use super::*;

    #[test]
    fn other_peer_version_shown_online_and_offline_alike() {
        // given: an online and an offline member, both have a version in the map
        let my_id = pair(1);
        let online = pair(2);
        let offline = pair(3);
        let mut inputs = base(my_id);
        inputs.members = [online].into_iter().collect();
        inputs.telemetry.peers.insert(offline, offline_peer());
        inputs.program_versions = versions([(online, "0.2.0"), (offline, "0.0.9")]);
        // when: the member list is built
        let peers = build_peers(&inputs);
        // then: the version is shown for both
        assert_eq!(
            version_of(&peers, online),
            Some("0.2.0".to_string()),
            "another member online with a record — Some"
        );
        assert_eq!(
            version_of(&peers, offline),
            Some("0.0.9".to_string()),
            "another member offline with a record — also Some"
        );
    }

    #[test]
    fn peer_without_record_has_no_version() {
        // given: two online members, the version is recorded for only one of them
        let my_id = pair(1);
        let with_record = pair(2);
        let without_record = pair(3);
        let mut inputs = base(my_id);
        inputs.members = [with_record, without_record].into_iter().collect();
        inputs.program_versions = versions([(with_record, "0.2.0")]);
        // when: the member list is built
        let peers = build_peers(&inputs);
        // then: the member without a record has no version, the neighbor has one
        assert_eq!(
            version_of(&peers, with_record),
            Some("0.2.0".to_string()),
            "a record exists — Some"
        );
        assert_eq!(
            version_of(&peers, without_record),
            None,
            "no record — None"
        );
    }
}

mod json {
    use super::*;

    #[test]
    fn peer_info_json_serializes_version_as_null_or_string() {
        // given: `PeerInfo` without a version and with a version
        let without = PeerInfo {
            program_version: None,
            ..Default::default()
        };
        let with = PeerInfo {
            program_version: Some("0.1.0".to_string()),
            ..Default::default()
        };
        // when: they are serialized to JSON
        let without_json = serde_json::to_value(without).unwrap();
        let with_json = serde_json::to_value(with).unwrap();
        // then: the key is called `version`: `null` without a version and a string with one
        assert_eq!(
            without_json.get("version"),
            Some(&serde_json::Value::Null),
            "no version — the key exists and equals null (without skip_serializing_if)"
        );
        assert_eq!(
            with_json.get("version"),
            Some(&serde_json::Value::String("0.1.0".to_string())),
            "a known version — a string"
        );
        assert_eq!(
            without_json.get("program_version"),
            None,
            "the Rust name does not leak into JSON"
        );
        assert_eq!(
            with_json.get("program_version"),
            None,
            "the Rust name does not leak into JSON"
        );
    }

    #[test]
    fn chosen_exit_json_has_no_version_key() {
        // given: a chosen exit whose version is in the map
        let my_id = pair(1);
        let exit = pair(2);
        let mut inputs = base(my_id);
        inputs.members = [exit].into_iter().collect();
        inputs.exits = [(exit, true)].into_iter().collect();
        inputs.chosen_exits = vec![exit];
        inputs.program_versions = versions([(exit, "3.2.1")]);
        // when: the snapshot is serialized to JSON
        let json = serde_json::to_value(build(inputs).snap).unwrap();
        // then: the member row has the version, while `route.exits[0]` does not
        let row = json["peers"]
            .as_array()
            .and_then(|rows| rows.iter().find(|p| p["id_full"] == exit.to_string()))
            .expect("the chosen exit should be in peers");
        assert_eq!(row["version"], "3.2.1", "the exit's version is in peers[]");
        let chosen = json["route"]["exits"][0]
            .as_object()
            .expect("the chosen exit in route.exits");
        assert_eq!(
            chosen["id_full"],
            exit.to_string(),
            "route.exits[0] is this exit"
        );
        assert!(
            !chosen.contains_key("version"),
            "route.exits[] carries no version"
        );
        assert!(
            !chosen.contains_key("program_version"),
            "route.exits[] carries no version"
        );
    }
}

fn offline_peer() -> PeerLive {
    PeerLive {
        online: false,
        path: None,
        rtt_ms: None,
        last_seen: Instant::now(),
    }
}

/// A version map made from "member, string" pairs.
fn versions<const N: usize>(pairs: [(PeerId, &str); N]) -> HashMap<PeerId, String> {
    pairs
        .into_iter()
        .map(|(id, v)| (id, v.to_string()))
        .collect()
}

/// The version in the row of member `id`; no row — the test fails.
fn version_of(peers: &[PeerInfo], id: PeerId) -> Option<String> {
    peers
        .iter()
        .find(|p| p.id == output::short_peer_id(&id))
        .unwrap_or_else(|| panic!("no row for {id}"))
        .program_version
        .clone()
}
