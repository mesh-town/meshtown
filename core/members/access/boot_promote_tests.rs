//! Tests of the one-time promotion of own witnesses to votes (`boot.rs`, `boot_plan::promotions`).
//! Child of `boot_tests.rs`: `Rig`, `setup`, `NOW` and the other helpers are there.

use super::*;

use crate::members::access::record::{encode, exclude_key, withdraw, BOOT_NOTE_ADMISSION};

/// A marker as the first stage wrote it: no flag.
async fn given_old_marker(rig: &Rig) {
    rig.given_marker(Via::Migration, NOW - DAY).await;
}

async fn flagged(rig: &Rig) -> bool {
    rig.marker()
        .await
        .is_some_and(|m| m.note == BOOT_NOTE_ADMISSION)
}

mod timing {
    use super::*;

    #[tokio::test]
    async fn old_timer_promotes_its_witnesses_once() {
        // given: a node of the first stage with a marker and witnesses for x and y
        let rig = setup();
        let (x, y) = (pid(2), pid(3));
        given_old_marker(&rig).await;
        rig.given_own_admit(x, Via::Witness).await;
        rig.given_own_admit(y, Via::Witness).await;
        // when: the first boot of this version
        rig.when_boot(&[], &[], NOW).await;
        // then: both are votes, the marker carries the flag, its origin and time are as they were
        assert_eq!(
            rig.admits().await,
            admits_of(&[(x, Via::None), (y, Via::None)])
        );
        let marker = rig.marker().await.expect("the marker stays");
        assert_eq!(
            (marker.note.as_str(), marker.via, marker.made_unix),
            (BOOT_NOTE_ADMISSION, Via::Migration, NOW - DAY)
        );
    }

    #[tokio::test]
    async fn second_boot_does_not_promote_again() {
        // given: a promoted node that wrote a new witness for z afterwards
        let rig = setup();
        let z = pid(4);
        given_old_marker(&rig).await;
        rig.when_boot(&[], &[], NOW).await;
        rig.given_own_admit(z, Via::Witness).await;
        // when: the next boot
        rig.when_boot(&[], &[], NOW + 60).await;
        // then: z stays a witness
        assert_eq!(rig.admits().await, admits_of(&[(z, Via::Witness)]));
    }

    #[tokio::test]
    async fn newcomer_promotes_nothing() {
        // given: a node with no earlier life and a neighbour from the invitation
        let rig = setup();
        let x = pid(2);
        // when: the first boot
        rig.when_boot(&[x], &[], NOW).await;
        // then: the witness stays a witness; the marker is flagged from the start
        assert_eq!(rig.admits().await, admits_of(&[(x, Via::Witness)]));
        let marker = rig.marker().await.expect("the marker is written");
        assert_eq!(
            (marker.via, marker.note.as_str()),
            (Via::None, BOOT_NOTE_ADMISSION)
        );
    }
}

mod refusals {
    use super::*;

    #[tokio::test]
    async fn withdrawn_admit_is_not_promoted_after_unadmit() {
        // given: an old marker, a withdrawn admit of x that is still a cached neighbour, a witness for y
        let rig = setup();
        let (x, y) = (pid(2), pid(3));
        given_old_marker(&rig).await;
        rig.given_own_admit(x, Via::Invite).await;
        withdraw(&*rig.shared, AclKind::Admit, x).await.unwrap();
        rig.given_own_admit(y, Via::Witness).await;
        // when: the boot
        rig.when_boot(&[], &[x], NOW).await;
        // then: x has no live record, y is a vote
        assert_eq!(rig.admits().await, admits_of(&[(y, Via::None)]));
    }

    #[tokio::test]
    async fn excluded_and_blocked_are_not_promoted() {
        // given: witnesses for an excluded node e and a blocked node b
        let rig = setup();
        let (v1, v2, e, b) = (pid(2), pid(3), pid(4), pid(5));
        given_old_marker(&rig).await;
        rig.given_own_admit(v1, Via::Invite).await;
        rig.given_own_admit(v2, Via::Invite).await;
        rig.given_own_admit(e, Via::Witness).await;
        rig.given_own_admit(b, Via::Witness).await;
        given_vote(&rig, rig.me, e, 10);
        rig.given_block(b).await;
        // when: the boot
        rig.when_boot(&[], &[], NOW).await;
        // then: both stay witnesses; the flag is set
        let want = admits_of(&[
            (v1, Via::Invite),
            (v2, Via::Invite),
            (e, Via::Witness),
            (b, Via::Witness),
        ]);
        assert_eq!(rig.admits().await, want);
        assert!(flagged(&rig).await);
    }
}

fn given_vote(rig: &Rig, voter: PeerId, subject: PeerId, ts_micros: u64) {
    let value = AclValue {
        kind: AclKind::Exclude,
        subject,
        via: Via::None,
        made_unix: 111,
        note: String::new(),
    };
    let value = Bytes::from(encode(&value));
    rig.shared.put_raw(ReplicaEntry {
        author: voter,
        key: exclude_key(&subject),
        value,
        ts_micros,
    });
}
