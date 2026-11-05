//! Admission tests of `admit_owner`: the label in a managed network and the stamp after an exclusion.

use super::*;

mod managed_net_note {
    use super::*;

    /// The note of the admit written for `pid(7)` when `entries` are in the document and `pid(5)` is the admin.
    async fn note_with(entries: &[ReplicaEntry], admins: Option<&BTreeSet<PeerId>>) -> String {
        let node = setup().await;
        entries.iter().for_each(|e| node.mem.put_raw(e.clone()));
        admit_owner(&node.list, &invite(1).hash_hex(), pid(7), None, NOW, admins)
            .await
            .unwrap();
        node.admit_of(pid(7)).await.expect("no admit record").note
    }

    fn admin_and_member() -> [ReplicaEntry; 2] {
        [
            v2_entry(pid(5), pid(5), NOW - 100, "Alice"),
            v2_entry(pid(6), pid(6), NOW - 10, "Mallory"),
        ]
    }

    #[tokio::test]
    async fn admit_owner_in_a_managed_net_ignores_a_member_label() {
        // given: an admin's older record "Alice" and a newer record "Mallory" of a plain member
        let admins = BTreeSet::from([pid(5)]);
        // when
        let note = note_with(&admin_and_member(), Some(&admins)).await;
        // then: the label of the admin's record
        assert_eq!(note, "Alice");
    }

    #[tokio::test]
    async fn admit_owner_in_a_managed_net_with_only_member_labels_leaves_the_note_empty() {
        // given: only a member's record
        let admins = BTreeSet::from([pid(5)]);
        // when
        let note = note_with(&admin_and_member()[1..], Some(&admins)).await;
        // then
        assert_eq!(note, "");
    }

    #[tokio::test]
    async fn admit_owner_without_admins_keeps_the_newest_label() {
        // given: the same two records in a network without admins
        // when
        let note = note_with(&admin_and_member(), None).await;
        // then: the newest one wins, as before
        assert_eq!(note, "Mallory");
    }
}

mod after_an_exclusion {
    use super::*;

    /// The admit time of `pid(7)` after admitting it twice: first at `NOW`, then with `after` at `later`.
    async fn second_admit_time(after: Option<u64>, later: u64) -> u64 {
        let node = setup().await;
        let hash = invite(1).hash_hex();
        admit_owner(&node.list, &hash, pid(7), None, NOW, None)
            .await
            .unwrap();
        admit_owner(&node.list, &hash, pid(7), after, later, None)
            .await
            .unwrap();
        assert_eq!(node.live_admits().await, 1);
        node.admit_of(pid(7))
            .await
            .expect("no admit record")
            .made_unix
    }

    #[tokio::test]
    async fn writes_a_second_after_it() {
        // given: an owner excluded at NOW + 500 while the clock is still behind it
        // when: it comes back by an invite
        let made = second_admit_time(Some(NOW + 500), NOW + 10).await;
        // then: the new record is one second later than the exclusion
        assert_eq!(made, NOW + 501);
    }

    #[tokio::test]
    async fn rewrites_an_older_admit() {
        // given: an owner admitted at NOW and excluded at NOW + 5
        // when: it comes back well after the exclusion
        let made = second_admit_time(Some(NOW + 5), NOW + 100).await;
        // then: the old record is replaced by one made at the clock
        assert_eq!(made, NOW + 100);
    }

    #[tokio::test]
    async fn keeps_now_when_now_is_later() {
        // given: a node with no admit of the owner and an exclusion older than the clock
        let node = setup().await;
        // when: the owner is admitted with the exclusion time
        admit_owner(
            &node.list,
            &invite(1).hash_hex(),
            pid(7),
            Some(NOW + 5),
            NOW + 100,
            None,
        )
        .await
        .unwrap();
        // then: the record carries the current time
        let made = node
            .admit_of(pid(7))
            .await
            .expect("no admit record")
            .made_unix;
        assert_eq!(made, NOW + 100);
    }
}
