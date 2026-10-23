//! Rules for `build_ports`: own publications, network ports, ports opened here.

use super::*;

use crate::transport::net::{NodeSecret, PeerId};

use crate::ports_sharing::{Audience, PortRecord};

fn id(n: u8) -> PeerId {
    NodeSecret::from_bytes(&[n; 32]).peer_id()
}

fn view(author: PeerId, port: u16, audience: Option<Audience>) -> PortView {
    PortView {
        author,
        port,
        record: audience.map(|audience| PortRecord {
            v: 1,
            name: "web".into(),
            audience,
        }),
    }
}

fn opened(node: PeerId, port: u16, local: u16) -> Opened {
    Opened { node, port, local }
}

const ME: u8 = 1;
const A: u8 = 2;

fn run(views: &[PortView], online: &[u8], op: OpenedNow) -> PortsInfo {
    let online: HashSet<PeerId> = online.iter().map(|n| id(*n)).collect();
    build_ports(views, id(ME), &HashMap::new(), &online, vec![], op)
}

#[test]
fn own_published_carry_local_state() {
    let p = |port| Published {
        port,
        name: "x".into(),
        audience: Audience::All,
    };
    let own = vec![(p(80), LocalState::Ok), (p(81), LocalState::Refused)];
    let info = build_ports(&[], id(ME), &HashMap::new(), &HashSet::new(), own, vec![]);
    let states: Vec<_> = info.published.iter().map(|o| o.local.as_str()).collect();
    assert_eq!(states, ["ok", "refused"]);
}

#[test]
fn foreign_all_record_of_online_author_is_listed() {
    let info = run(&[view(id(A), 80, Some(Audience::All))], &[A], vec![]);
    assert_eq!(info.network.len(), 1);
    assert!(info.network[0].online && info.network[0].published);
    assert_eq!(info.network[0].opened, None);
}

#[test]
fn offline_author_not_opened_has_no_row() {
    let info = run(&[view(id(A), 80, Some(Audience::All))], &[], vec![]);
    assert!(info.network.is_empty());
}

#[test]
fn offline_author_opened_here_keeps_row() {
    let op = vec![(opened(id(A), 80, 9000), OpenState::Listening)];
    let info = run(&[view(id(A), 80, Some(Audience::All))], &[], op);
    assert_eq!(info.network.len(), 1);
    assert!(!info.network[0].online);
    assert!(info.network[0].published);
}

#[test]
fn nodes_without_me_or_missing_record_not_opened_has_no_row() {
    let views = [
        view(id(A), 80, Some(Audience::Nodes(vec![id(3)]))),
        view(id(A), 81, None),
    ];
    assert!(run(&views, &[A], vec![]).network.is_empty());
}

#[test]
fn nodes_without_me_or_missing_record_opened_here_is_unpublished_row() {
    let views = [
        view(id(A), 80, Some(Audience::Nodes(vec![id(3)]))),
        view(id(A), 81, None),
    ];
    let op = vec![
        (opened(id(A), 80, 9000), OpenState::Listening),
        (opened(id(A), 81, 9001), OpenState::Listening),
    ];
    let info = run(&views, &[A], op);
    assert_eq!(info.network.len(), 2);
    assert!(info
        .network
        .iter()
        .all(|r| !r.published && r.opened.is_some()));
}

#[test]
fn nodes_record_listing_me_is_published_row() {
    let views = [view(id(A), 80, Some(Audience::Nodes(vec![id(ME)])))];
    let info = run(&views, &[A], vec![]);
    assert_eq!(info.network.len(), 1);
    assert!(info.network[0].published);
    assert_eq!(info.network[0].name, "web");
}

#[test]
fn nodes_record_listing_me_offline_author_no_row() {
    let views = [view(id(A), 80, Some(Audience::Nodes(vec![id(ME)])))];
    assert!(run(&views, &[], vec![]).network.is_empty());
}

#[test]
fn empty_nodes_record_has_no_row() {
    let views = [view(id(A), 80, Some(Audience::Nodes(vec![])))];
    assert!(run(&views, &[A], vec![]).network.is_empty());
}

#[test]
fn own_record_is_not_in_network() {
    let info = run(&[view(id(ME), 80, Some(Audience::All))], &[ME], vec![]);
    assert!(info.network.is_empty());
}

#[test]
fn opened_without_record_is_unpublished_row() {
    let op = vec![(opened(id(A), 80, 9000), OpenState::Listening)];
    let info = run(&[], &[A], op);
    assert_eq!(info.network.len(), 1);
    assert!(!info.network[0].published);
    let o = info.network[0].opened.as_ref().unwrap();
    assert_eq!((o.local, o.state.as_str()), (9000, "listening"));
}

#[test]
fn bind_failed_carries_error_text() {
    let op = vec![(
        opened(id(A), 80, 9000),
        OpenState::BindFailed("busy".into()),
    )];
    let info = run(&[view(id(A), 80, Some(Audience::All))], &[A], op);
    let o = info.network[0].opened.as_ref().unwrap();
    assert_eq!(o.state, "bind_failed");
    assert_eq!(o.error.as_deref(), Some("busy"));
}

#[path = "ports_web_url_tests.rs"]
mod web_url;
