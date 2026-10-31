//! Admitting nodes (`actions.rs`); helpers are in `actions_tests.rs`.

use super::*;

#[tokio::test]
async fn admit_of_a_member_changes_nothing() {
    // given: a is on the list
    let world = setup(&[pid(2)]);
    world.refresh().await;
    let before = world.shared_entries().await;
    // when / then: admitting a again is a no-op that writes nothing
    assert_eq!(world.apply(MemberAction::Admit(pid(2))).await, Ok(false));
    assert_eq!(world.shared_entries().await, before);
}

#[tokio::test]
async fn admit_of_an_unlisted_node_is_still_refused() {
    // given: the stranger is neither waiting nor admitted
    let world = setup(&[pid(2)]);
    // when / then
    assert_eq!(
        world.apply(MemberAction::Admit(pid(9))).await,
        Err(ActionRefusal::NotPending)
    );
}
