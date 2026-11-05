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
            collected_at: tokio::time::Instant::now(),
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

fn fresh_collected() -> Collected {
    Collected {
        snap: Default::default(),
        domains: vec![],
        partial: false,
        collected_at: tokio::time::Instant::now(),
    }
}

mod stale_read {
    use super::*;
    use std::time::Duration;

    use tokio::time::{advance, timeout};

    use super::super::store_snapshot;
    use crate::config::config_file::Settings;
    use crate::node::node_ctl::state::NodeCtl;
    use crate::web_gui::NodeControl;

    fn ctl_with_cache() -> std::sync::Arc<NodeCtl> {
        let ctl = NodeCtl::new_without_network(&Settings::default());
        ctl.ctl_shared.ctl_state.lock().unwrap().cache = Some(fresh_collected());
        ctl
    }

    async fn kicked(ctl: &NodeCtl) -> bool {
        let wait = ctl.ctl_shared.kick.notified();
        timeout(Duration::from_millis(1), wait).await.is_ok()
    }

    fn store_new_collection(ctl: &NodeCtl) {
        let generation = ctl.ctl_shared.ctl_state.lock().unwrap().snapshot_gen;
        store_snapshot(&ctl.ctl_shared, generation, fresh_collected());
    }

    #[tokio::test(start_paused = true)]
    async fn stale_read_wakes_collection_and_fresh_snapshot_follows() {
        // given: a cached snapshot and a page idle for more than 5 s
        let ctl = ctl_with_cache();
        advance(Duration::from_secs(6)).await;
        // when: the page reads
        let first = ctl.snapshot();
        // then: the old snapshot comes back and the task is woken
        assert!(first.snapshot_age_ms >= 6_000);
        assert!(kicked(&ctl).await);
        // and when: the task stores a new collection
        store_new_collection(&ctl);
        // then: the next read is fresh and wakes nothing
        assert!(ctl.snapshot().snapshot_age_ms < 1_000);
        assert!(!kicked(&ctl).await);
    }

    #[tokio::test(start_paused = true)]
    async fn snapshot_age_grows_until_the_next_collection() {
        // given: a fresh collection
        let ctl = ctl_with_cache();
        assert_eq!(ctl.snapshot().snapshot_age_ms, 0);
        // when: time passes with no collection
        advance(Duration::from_secs(3)).await;
        // then: the age grows
        assert_eq!(ctl.snapshot().snapshot_age_ms, 3_000);
        // and when: a collection lands
        store_new_collection(&ctl);
        // then: the age starts over
        assert_eq!(ctl.snapshot().snapshot_age_ms, 0);
    }
}
