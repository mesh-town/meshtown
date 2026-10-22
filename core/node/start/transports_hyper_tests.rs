use super::*;
use crate::transport::net::service::{AUTH, EXIT, INVITE, PORT, REPLICA, SYNC};
use crate::transport::net::PeerId;

fn pid(n: u8) -> PeerId {
    PeerId::from_bytes(&[n; 32]).unwrap()
}

const OTHERS: [crate::transport::net::service::ServiceId; 4] = [EXIT, PORT, SYNC, REPLICA];

/// A foreign key: AUTH and INVITE pass.
#[test]
fn stranger_auth_and_invite_pass() {
    // given: an empty member list
    let admit = member_admit(Members::default());
    // then: AUTH and INVITE are let in
    assert!(admit(pid(1), &AUTH));
    assert!(admit(pid(1), &INVITE));
}

/// The window before `admit_members_on_hyper`: only AUTH and INVITE, even if the key were known.
#[test]
fn window_before_wire_admits_only_auth_invite() {
    // given: the window predicate
    let admit = auth_invite_only();
    // then: AUTH and INVITE — yes, the others — no
    assert!(admit(pid(1), &AUTH) && admit(pid(1), &INVITE));
    for svc in OTHERS {
        assert!(
            !admit(pid(1), &svc),
            "service {svc:?} let in during the window"
        );
    }
}

/// A foreign key doesn't pass to the other services.
#[test]
fn stranger_other_services_refused() {
    let admit = member_admit(Members::default());
    for svc in OTHERS {
        assert!(
            !admit(pid(1), &svc),
            "service {svc:?} let in for a foreign key"
        );
    }
}

/// A member passes to any service.
#[test]
fn member_passes_any_service() {
    let members = Members::default();
    members.add_verified_conn(pid(1));
    let admit = member_admit(members);
    for svc in OTHERS {
        assert!(admit(pid(1), &svc), "service {svc:?} closed to a member");
    }
}

/// The predicate is live: a member added after it was created passes.
#[test]
fn member_added_later_passes() {
    // given: a predicate over an empty list
    let members = Members::default();
    let admit = member_admit(members.clone());
    assert!(!admit(pid(3), &EXIT));
    // when: after AUTH the key became a member
    members.add_verified_conn(pid(3));
    // then: the same predicate lets it in
    assert!(admit(pid(3), &EXIT));
}
