//! Tests of `snapshot_self_name` and `with_previous_domains`.

use super::{snapshot_self_name, with_previous_domains, Collected};

#[test]
fn snapshot_self_name_prefers_effective_settings() {
    assert_eq!(snapshot_self_name(Some("x"), "y"), "x");
}

#[test]
fn snapshot_self_name_falls_back_to_running_node() {
    assert_eq!(snapshot_self_name(None, "y"), "y");
}

mod with_previous_domains {
    use super::*;

    fn collected(domains: &[&str], partial: bool) -> Collected {
        Collected {
            snap: Default::default(),
            domains: domains.iter().map(|d| d.to_string()).collect(),
            partial,
        }
    }

    #[test]
    fn partial_takes_previous() {
        let prev = collected(&["a"], false);
        let out = with_previous_domains(collected(&[], true), Some(&prev));
        assert_eq!(out.domains, ["a"]);
    }

    #[test]
    fn full_keeps_own() {
        let prev = collected(&["a"], false);
        let out = with_previous_domains(collected(&["b"], false), Some(&prev));
        assert_eq!(out.domains, ["b"]);
    }

    #[test]
    fn no_previous_keeps_own() {
        let out = with_previous_domains(collected(&["b"], true), None);
        assert_eq!(out.domains, ["b"]);
    }
}
