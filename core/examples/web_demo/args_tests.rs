use clap::Parser;

use super::Args;
use crate::managed::DemoRole;
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

#[test]
fn the_role_flags_pick_the_demo_role() {
    let role = |flags: &[&str]| {
        let mut argv = vec!["web_demo"];
        argv.extend_from_slice(flags);
        Args::parse_from(argv).demo_role()
    };
    assert_eq!(role(&[]), DemoRole::Root);
    assert_eq!(role(&["--second-admin"]), DemoRole::SecondAdmin);
    assert_eq!(role(&["--role", "member"]), DemoRole::Member);
}
