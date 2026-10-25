use clap::Parser;

use super::Args;
use crate::node::build_demo_peers;

#[test]
fn flags_default_to_off() {
    let args = Args::parse_from(["web_demo"]);
    assert!(!args.admission_refused);
    assert_eq!(args.no_exit_peers, 0);
}

#[test]
fn flags_are_parsed() {
    let args = Args::parse_from(["web_demo", "--admission-refused", "--no-exit-peers=1"]);
    assert!(args.admission_refused);
    assert_eq!(args.no_exit_peers, 1);
}

#[test]
fn the_last_peers_do_not_offer_an_exit() {
    let exits: Vec<bool> = build_demo_peers(0, 1, &mut 1)
        .iter()
        .map(|p| p.exit)
        .collect();
    assert_eq!(exits, [true, false]);
}
