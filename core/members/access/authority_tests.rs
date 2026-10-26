//! Tests of the records of power (`authority.rs`).

use super::*;

#[test]
fn an_admission_keeps_the_origin_it_came_with() {
    // given: admissions of every origin
    // when / then: the power carries the same origin
    for via in [Via::None, Via::Invite, Via::Migration, Via::Witness] {
        assert_eq!(power_of(AclKind::Admit, via), Some(Power::Admit(via)));
    }
}

#[test]
fn grant_revoke_and_close_are_powers_whatever_the_origin() {
    // given: the three kinds with an origin that means nothing for them
    // when / then
    assert_eq!(power_of(AclKind::Grant, Via::Invite), Some(Power::Grant));
    assert_eq!(power_of(AclKind::Revoke, Via::None), Some(Power::Revoke));
    assert_eq!(power_of(AclKind::Close, Via::Witness), Some(Power::Close));
}

#[test]
fn exclusions_blocks_and_the_boot_marker_are_not_powers() {
    // given: the kinds that are no records of power
    // when / then
    for kind in [AclKind::Exclude, AclKind::Block, AclKind::Boot] {
        assert_eq!(power_of(kind, Via::None), None, "{kind:?}");
    }
}
