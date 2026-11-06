use super::*;

#[test]
fn default_choice_equals_net_choice_default() {
    // given: a node build, all default names are built
    // when: the node asks for its build's default
    let choice = default_choice();
    // then: it matches the core default: hyper, invite+cache+hyperdht, mesh
    assert_eq!(choice, crate::transport::net::NetChoice::default());
    assert_eq!(choice.transports, vec!["hyper"]);
    assert_eq!(choice.discovery, vec!["invite", "cache", "hyperdht"]);
    assert_eq!(choice.sync, "mesh");
}

#[test]
fn sync_names_are_mem_and_mesh() {
    // given: any node build
    // when: the node asks for the built syncs
    // then: two — in memory and its own mesh, in that order
    assert_eq!(sync(), vec!["mem", "mesh"]);
}

#[test]
fn transports_are_hyper_only() {
    // given: any node build
    // when: the node asks for the built transports
    // then: only hyper; mem can't be chosen from the config
    assert_eq!(transports(), vec!["hyper"]);
}

#[test]
fn discovery_has_hyperdht_and_no_mainline() {
    // given: any node build
    // when: the node asks for the built discovery sources
    let names = discovery();
    // then: invite, cache, hyperdht; no mainline
    assert_eq!(names, vec!["invite", "cache", "hyperdht"]);
}
