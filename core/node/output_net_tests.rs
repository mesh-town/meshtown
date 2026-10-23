//! Tests of the network label in the log: `with_net`, filtering and the shown flag.

use super::*;
use std::sync::Arc;

fn label(id: &str) -> Option<Arc<str>> {
    Some(Arc::from(id))
}

fn nets(j: &Journal, filter: Option<&str>) -> Vec<(String, Option<String>, bool)> {
    j.page("", 0, 100, filter)
        .lines
        .into_iter()
        .map(|l| (l.text, l.net.map(|n| n.to_string()), l.shown))
        .collect()
}

#[tokio::test]
async fn line_inside_with_net_carries_the_label() {
    let j = Journal::new(10);
    with_net(label("ab2c3d"), async { j.say("t", "x") }).await;
    assert_eq!(
        nets(&j, None),
        vec![("x".into(), Some("ab2c3d".into()), false)]
    );
}

#[tokio::test]
async fn line_without_label_has_none() {
    let j = Journal::new(10);
    j.say("t", "x");
    assert_eq!(nets(&j, None), vec![("x".into(), None, false)]);
    assert!(current_net().is_none());
}

#[tokio::test]
async fn filter_selects_by_stored_label_and_skips_process_lines() {
    let j = Journal::new(10);
    j.say("t", "process");
    with_net(label("ab2c3d"), async { j.say("t", "a") }).await;
    with_net(label("main"), async { j.say("t", "m") }).await;
    let texts = |f| nets(&j, f).into_iter().map(|l| l.0).collect::<Vec<_>>();
    assert_eq!(texts(None), vec!["process", "a", "m"]);
    assert_eq!(texts(Some("ab2c3d")), vec!["a"]);
    assert_eq!(texts(Some("main")), vec!["m"]);
}

#[tokio::test]
async fn single_network_stores_label_but_does_not_show_it() {
    let j = Journal::new(10);
    with_net(label("main"), async { j.say("t", "m") }).await;
    assert_eq!(
        nets(&j, Some("main")),
        vec![("m".into(), Some("main".into()), false)]
    );
}

#[tokio::test]
async fn multi_network_shows_label_and_later_switch_off_hides_it() {
    let j = Journal::new(10);
    j.set_multi_net(true);
    with_net(label("main"), async { j.say("t", "one") }).await;
    j.say("t", "process");
    j.set_multi_net(false);
    with_net(label("main"), async { j.say("t", "two") }).await;
    let got = nets(&j, None);
    assert!(got[0].2);
    assert!(!got[1].2);
    assert_eq!(got[2], ("two".into(), Some("main".into()), false));
}
