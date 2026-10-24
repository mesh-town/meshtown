//! Tests of the slot watcher (`closed_watch.rs`) for the main slot: it leaves in place and stays in the map.

use super::*;

#[tokio::test(start_paused = true)]
async fn leaves_in_place_and_notes_once() {
    // given: the main slot with a key from the settings file
    let fx = setup(Opts {
        main_key: true,
        ..Opts::default()
    });
    fx.spawn_main_cycle();
    // when
    fx.when_closed_main(Some(closing(5))).await;
    // then: the slot stays, a leave task runs, one note is made
    assert!(fx.slots.slot(&NetSlotId::main()).is_some());
    assert_eq!(fx.slots.take_retiring().len(), 1);
    assert_eq!(fx.notes().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_same_value_and_a_return_to_it_give_no_second_note() {
    // given: the first closing was handled
    let fx = setup(Opts {
        main_key: true,
        ..Opts::default()
    });
    fx.spawn_main_cycle();
    fx.when_closed_main(Some(closing(5))).await;
    // when: the same value again, then none and the same value once more
    fx.when_closed_main(Some(closing(5))).await;
    fx.when_closed_main(None).await;
    fx.when_closed_main(Some(closing(5))).await;
    // then: still one note and no second leave task
    assert_eq!(fx.notes().len(), 1);
    assert_eq!(fx.slots.take_retiring().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_new_closing_after_a_switch_fires_again() {
    // given: the closing of the first network was handled
    let fx = setup(Opts {
        main_key: true,
        ..Opts::default()
    });
    fx.spawn_main_cycle();
    fx.when_closed_main(Some(closing(5))).await;
    // when: the slot moves to another network (none in between) and that one is closed
    fx.when_closed_main(None).await;
    fx.when_closed_main(Some(closing(9))).await;
    // then: two deletions are noted
    let notes = fx.notes();
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert_eq!(notes[1].at_unix, 9);
}

#[tokio::test(start_paused = true)]
async fn a_key_from_a_flag_stays_and_the_deletion_is_noted() {
    // given: the key comes from --key, so the network cannot be left
    let fx = setup(Opts {
        main_key: true,
        key_from_flag: true,
    });
    fx.spawn_main_cycle();
    // when
    fx.when_closed_main(Some(closing(5))).await;
    // then: no leave task, the key is kept, one note
    assert!(fx.slots.take_retiring().is_empty());
    assert_eq!(fx.main_key(), Some(KEY.to_string()));
    assert_eq!(fx.notes().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_manual_leave_twice_and_a_closing_start_one_leave() {
    // given: the main network is being left by hand, the button pressed twice
    let fx = setup(Opts {
        main_key: true,
        ..Opts::default()
    });
    fx.spawn_main_cycle();
    fx.slots.retire(&NetSlotId::main(), false).unwrap();
    fx.slots.retire(&NetSlotId::main(), false).unwrap();
    // when: the closing arrives while the leave goes on
    fx.when_closed_main(Some(closing(5))).await;
    // then: only the first leave started a task
    assert_eq!(fx.slots.take_retiring().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_closing_and_then_a_manual_leave_start_one_leave() {
    // given: the closing started the leave
    let fx = setup(Opts {
        main_key: true,
        ..Opts::default()
    });
    fx.spawn_main_cycle();
    fx.when_closed_main(Some(closing(5))).await;
    // when: the button is pressed while it goes on
    fx.slots.retire(&NetSlotId::main(), false).unwrap();
    // then: no second task
    assert_eq!(fx.slots.take_retiring().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn spawning_the_cycle_again_replaces_the_watcher() {
    // given: the cycle of the main slot was spawned twice
    let fx = setup(Opts {
        main_key: true,
        ..Opts::default()
    });
    fx.spawn_main_cycle();
    fx.spawn_main_cycle();
    // when
    fx.when_closed_main(Some(closing(5))).await;
    // then: one watcher acted, so one note
    assert_eq!(fx.notes().len(), 1, "{:?}", fx.notes());
}
