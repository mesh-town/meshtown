use super::*;

#[test]
fn no_free_slot_refuses() {
    let slots = Arc::new(Semaphore::new(0));
    assert!(take_slot(&slots).is_none());
}

#[test]
fn a_slot_is_returned_when_the_permit_drops() {
    let slots = Arc::new(Semaphore::new(1));
    let permit = take_slot(&slots).expect("one free slot");
    assert!(take_slot(&slots).is_none());
    drop(permit);
    assert!(take_slot(&slots).is_some());
}

#[test]
fn node_slots_hold_the_limit() {
    assert_eq!(udp_slots().available_permits(), MAX_UDP_ASSOCIATIONS);
}
