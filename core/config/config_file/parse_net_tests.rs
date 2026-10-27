use crate::config::config_file::{parse_text, ParsedConfigFile, RelayMode};
use crate::node::net_names;
use crate::transport::net::NetChoice;

#[test]
fn missing_section_is_default() {
    // given: config without a net section
    // when: parsing
    let p = parse_without_net();
    // then: the network choice is the build default, no field errors
    assert_eq!(p.settings.net.net_choice, net_names::default_choice());
    assert_eq!(p.field_errors, vec![]);
}

#[test]
fn empty_object_is_default() {
    // given: "net": {}
    // when: parsing
    let p = parse_net("{}");
    // then: the same as without the section
    assert_eq!(p.settings.net.net_choice, net_names::default_choice());
    assert_eq!(p.field_errors, vec![]);
}

#[test]
fn sync_mem_taken() {
    // given: "sync": "mem"
    // when: parsing
    let p = parse_net(r#"{"sync": "mem"}"#);
    // then: sync is taken, the lists are defaults, no errors or unknown keys
    let d = net_names::default_choice();
    assert_eq!(p.settings.net.net_choice.sync, "mem");
    assert_eq!(p.settings.net.net_choice.transports, d.transports);
    assert_eq!(p.settings.net.net_choice.discovery, d.discovery);
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "sync is a known key: {:?}",
        p.unknown_paths
    );
}

#[test]
fn sync_mesh_is_accepted() {
    // given: "sync": "mesh"
    // when: parsing
    let p = parse_net(r#"{"sync": "mesh"}"#);
    // then: sync is taken with no warnings or unknown keys
    assert_eq!(p.settings.net.net_choice.sync, "mesh");
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "sync is a known key: {:?}",
        p.unknown_paths
    );
}

#[test]
fn unknown_sync_lists_mesh() {
    // given: "sync": "meshh" — a typo
    // when: parsing
    let p = parse_net(r#"{"sync": "meshh"}"#);
    // then: the default mesh is taken, the list of built-in ones includes mesh
    assert_eq!(p.settings.net.net_choice.sync, "mesh");
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    let message = &p.field_errors[0].message;
    assert!(
        message.contains("mem, mesh"),
        "no list of built-in ones: {message}"
    );
}

#[test]
fn sync_unknown_name_is_field_error() {
    // given: "sync": "foo"
    // when: parsing
    let p = parse_net(r#"{"sync": "foo"}"#);
    // then: one net.sync error with the list of built-in ones and the default, sync is the default
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    let err = &p.field_errors[0];
    assert_eq!(err.field_path, "net.sync");
    assert!(
        err.message.contains("mem, mesh"),
        "no list of built-in ones: {}",
        err.message
    );
    assert!(
        err.message.contains("using default mesh"),
        "no default: {}",
        err.message
    );
    assert_eq!(p.settings.net.net_choice.sync, "mesh");
}

#[test]
fn sync_not_string_is_field_error() {
    // given: "sync": 5
    // when: parsing
    let p = parse_net(r#"{"sync": 5}"#);
    // then: a net.sync error, sync is the default
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    assert_eq!(p.field_errors[0].field_path, "net.sync");
    assert_eq!(p.settings.net.net_choice.sync, "mesh");
}

#[test]
fn discovery_order_kept() {
    // given: "discovery": ["cache", "invite"] — the order is not the default
    // when: parsing
    let p = parse_net(r#"{"discovery": ["cache", "invite"]}"#);
    // then: the order is kept, no errors or unknown keys
    assert_eq!(p.settings.net.net_choice.discovery, vec!["cache", "invite"]);
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "discovery is a known key: {:?}",
        p.unknown_paths
    );
}

#[test]
fn discovery_empty_is_field_error() {
    // given: "discovery": []
    // when: parsing
    let p = parse_net(r#"{"discovery": []}"#);
    // then: an empty list is a field error, the field is the default
    assert_only_discovery_failed(&p);
}

#[test]
fn discovery_duplicate_is_field_error() {
    // given: "discovery": ["invite", "invite"]
    // when: parsing
    let p = parse_net(r#"{"discovery": ["invite", "invite"]}"#);
    // then: a duplicate is a field error, the field is the default
    assert_only_discovery_failed(&p);
}

#[test]
fn discovery_non_string_is_field_error() {
    // given: "discovery": ["invite", 3]
    // when: parsing
    let p = parse_net(r#"{"discovery": ["invite", 3]}"#);
    // then: a non-string in the list is a field error, the whole field is the default
    assert_only_discovery_failed(&p);
}

#[test]
fn discovery_unknown_name_lists_compiled() {
    // given: "discovery": ["dht"] — there is no such source
    // when: parsing
    let p = parse_net(r#"{"discovery": ["dht"]}"#);
    // then: a field error with the list of built-in names, the field is the default
    assert_only_discovery_failed(&p);
    let message = &p.field_errors[0].message;
    assert!(
        message.contains("invite, cache, hyperdht"),
        "no list of built-in ones: {message}"
    );
}

#[test]
fn transports_hyper_taken() {
    // given: "transports": ["hyper"]
    // when: parsing
    let p = parse_net(r#"{"transports": ["hyper"]}"#);
    // then: the list is taken, no errors or unknown keys
    assert_eq!(p.settings.net.net_choice.transports, vec!["hyper"]);
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "transports is a known key: {:?}",
        p.unknown_paths
    );
}

#[test]
fn transports_unknown_is_field_error() {
    // given: "transports": ["wormhole"] — no build has such a transport
    // when: parsing
    let p = parse_net(r#"{"transports": ["wormhole"]}"#);
    // then: one net.transports error with the list of built-in ones, the field is the default
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    let err = &p.field_errors[0];
    assert_eq!(err.field_path, "net.transports");
    assert!(
        err.message.contains("from: hyper"),
        "no list of built-in ones: {}",
        err.message
    );
    assert_eq!(p.settings.net.net_choice.transports, vec!["hyper"]);
}

#[test]
fn one_bad_field_keeps_others() {
    // given: sync is wrong, discovery is right
    // when: parsing
    let p = parse_net(r#"{"sync": "foo", "discovery": ["cache"]}"#);
    // then: the right field is taken, the wrong one is the default, one error
    assert_eq!(p.settings.net.net_choice.discovery, vec!["cache"]);
    assert_eq!(p.settings.net.net_choice.sync, "mesh");
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    assert_eq!(p.field_errors[0].field_path, "net.sync");
}

#[test]
fn unknown_key_in_net_reported() {
    // given: an unknown key foo in net
    // when: parsing
    let p = parse_net(r#"{"foo": 1}"#);
    // then: the path is in unknown, no field errors, the choice is the default
    assert!(
        p.unknown_paths.contains(&"config.net.foo".to_string()),
        "unknown: {:?}",
        p.unknown_paths
    );
    assert_eq!(p.field_errors, vec![]);
    assert_eq!(p.settings.net.net_choice, net_names::default_choice());
}

#[test]
fn relay_missing_is_network() {
    // given: a net section without the relay field, and a config without a net section
    // when: parsing
    let with_section = parse_net("{}");
    let without_section = parse_without_net();
    // then: relaying through network members is on, nothing is reported
    assert_eq!(with_section.settings.net.relay, RelayMode::Network);
    assert_eq!(without_section.settings.net.relay, RelayMode::Network);
    assert_eq!(with_section.field_errors, vec![]);
    assert_eq!(without_section.field_errors, vec![]);
}

#[test]
fn relay_off_taken() {
    // given: "relay": "off"
    // when: parsing
    let p = parse_net(r#"{"relay": "off"}"#);
    // then: relay is off, relay is a known key, the other fields stay default
    assert_eq!(p.settings.net.relay, RelayMode::Off);
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "relay is a known key: {:?}",
        p.unknown_paths
    );
    assert_eq!(p.settings.net.net_choice, net_names::default_choice());
}

#[test]
fn relay_network_taken() {
    // given: "relay": "network"
    // when: parsing
    let p = parse_net(r#"{"relay": "network"}"#);
    // then: relay is network with no complaints
    assert_eq!(p.settings.net.relay, RelayMode::Network);
    assert_eq!(p.field_errors, vec![]);
    assert!(
        p.unknown_paths.is_empty(),
        "relay is a known key: {:?}",
        p.unknown_paths
    );
}

#[test]
fn relay_unknown_is_error_and_default() {
    // given: "relay": "any" — a value this version does not know
    // when: parsing
    let p = parse_net(r#"{"relay": "any"}"#);
    // then: one net.relay error, the mode is the default, neighbors are untouched
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    assert_eq!(p.field_errors[0].field_path, "net.relay");
    assert_eq!(p.settings.net.relay, RelayMode::Network);
    assert_eq!(p.settings.net.net_choice, net_names::default_choice());
}

#[test]
fn relay_not_string_is_error_and_default() {
    // given: "relay": true
    // when: parsing
    let p = parse_net(r#"{"relay": true}"#);
    // then: one net.relay error, the mode is the default
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    assert_eq!(p.field_errors[0].field_path, "net.relay");
    assert_eq!(p.settings.net.relay, RelayMode::Network);
}

/// Parses a document where the `net` section is `net` (the text of a JSON value).
fn parse_net(net: &str) -> ParsedConfigFile {
    let text = format!(r#"{{"config": {{"version": 1, "net": {net}}}}}"#);
    parse_text(&text).expect("the document parsed")
}

fn parse_without_net() -> ParsedConfigFile {
    parse_text(r#"{"config": {"version": 1}}"#).expect("the document parsed")
}

/// Given a damaged `discovery`: exactly one `net.discovery` error, the field is the default,
/// the neighboring fields of the section are untouched.
fn assert_only_discovery_failed(p: &ParsedConfigFile) {
    let d: NetChoice = net_names::default_choice();
    assert_eq!(p.field_errors.len(), 1, "{:?}", p.field_errors);
    assert_eq!(p.field_errors[0].field_path, "net.discovery");
    assert_eq!(
        p.settings.net.net_choice.discovery, d.discovery,
        "the field should become the default"
    );
    assert_eq!(p.settings.net.net_choice.sync, d.sync);
    assert_eq!(p.settings.net.net_choice.transports, d.transports);
}
