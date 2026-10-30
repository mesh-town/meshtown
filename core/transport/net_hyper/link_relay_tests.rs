//! The relay label of a session (`relay_label`) and the route text built from it.

use super::*;
use crate::transport::net::contract::pid;
use townjump_dht::RelayPath;

mod relay_label {
    use super::*;

    #[test]
    fn relayed_through_a_member_names_it() {
        // given: the dial went through the relay of a member
        let member = pid(7);
        // when
        let label = relay_label(Some(member), true);
        // then
        assert_eq!(label, Some(RelayRef::Peer(member)));
    }

    #[test]
    fn relayed_without_a_member_is_unnamed() {
        // given: a relayed session whose relay is not known (the receiving side)
        // when
        let label = relay_label(None, true);
        // then
        assert_eq!(label, Some(RelayRef::Unnamed));
    }

    #[test]
    fn direct_link_has_no_relay_even_after_a_relay_phase() {
        // given: the dial ran a relay phase through a member, but the session is not relayed
        let member = pid(7);
        // when
        let label = relay_label(Some(member), false);
        // then
        assert_eq!(label, None, "a direct session must not carry a relay label");
    }

    #[test]
    fn direct_link_without_a_phase_has_none() {
        // given: a direct dial, no relay phase
        // when
        let label = relay_label(None, false);
        // then
        assert_eq!(label, None);
    }
}

mod route_text {
    use super::*;

    #[test]
    fn unnamed_relay_has_its_text() {
        // given: a relayed session whose relay is unknown
        // when
        let text = route_text(Some(RelayRef::Unnamed));
        // then
        assert_eq!(text, "relayed via a relay server");
    }
}

mod relay_now {
    use super::*;

    #[test]
    fn relay_now_drops_the_relay_after_the_switch() {
        // given: a session dialed through a member's relay, still relayed
        let member = pid(7);
        let (tx, facts) = relayed_facts(RelayRef::Peer(member));
        assert_eq!(
            facts.relay_now(),
            Some(RelayRef::Peer(member)),
            "relayed: the label is kept"
        );
        // when: the connection moves to the direct path
        tx.send(RelayPath::Direct).unwrap();
        // then: the session no longer carries a relay label
        assert_eq!(
            facts.relay_now(),
            None,
            "direct after the switch: no relay label"
        );
    }

    #[test]
    fn relay_now_of_a_direct_session_is_none() {
        // given: a session that was never relayed
        let facts = PathFacts::default();
        // when / then
        assert_eq!(facts.relay_now(), None);
    }
}

/// Path facts of a session on `relay`, with a live path source that starts at `Relayed`.
fn relayed_facts(relay: RelayRef) -> (tokio::sync::watch::Sender<RelayPath>, PathFacts) {
    let (tx, rx) = tokio::sync::watch::channel(RelayPath::Relayed);
    let facts = PathFacts {
        relay: Some(relay),
        relay_path: Some(rx),
        ..Default::default()
    };
    (tx, facts)
}
