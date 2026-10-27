//! Tests of the invite wait: the key arrives after the wait was cancelled (split from `invite_wait_tests.rs`).

use super::super::Job;
use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accept_does_not_call_save_when_the_wait_was_replaced() {
    // given: a wait is running, its generation is remembered, then the wait is cleared by a network switch
    let fx = setup_waiting();
    let (invite_gen, invite) = {
        let st = fx.ctl.ctl_shared.ctl_state.lock().unwrap();
        (
            st.invite_gen,
            st.invite_wait.as_ref().unwrap().view.label.clone(),
        )
    };
    assert_eq!(invite.as_deref(), Some("Alice"));
    fx.ctl.switch_network("mesh2-aaaa");
    assert_eq!(fx.ctl.invite_join(), None);

    // when: the key arrives after the cancel
    let job = Job {
        ctl_shared: fx.ctl.ctl_shared.clone(),
        cmd_tx: fx.ctl.cmd_tx.clone(),
        invite_gen,
        head: head(None, now()),
        peers: vec![],
        save: fx.recording_save(),
        test_net: None,
        owner: None,
    };
    job.accept("mesh2-late".to_string(), None);

    // then: the settings file is untouched
    assert_eq!(
        fx.saved(),
        Vec::<String>::new(),
        "save was called after the wait was cancelled"
    );
}
