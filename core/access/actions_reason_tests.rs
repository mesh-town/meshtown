//! Tests of the reason noted with an exclusion vote and of taking a proposal back (part of `actions_tests.rs`;
//! its `World` helpers are used here). Nodes: `me` (seed 1), `x` (seed 2) and `b` (seed 3) are on the list,
//! so two votes are needed to exclude one of them.

use super::*;
use crate::access::report::members_reply;
use crate::web::MemberRow;

const LATE: u64 = 4_000_000_000_000_000;

mod note {
    use super::*;

    #[tokio::test]
    async fn exclude_writes_the_note_into_my_vote_record() {
        // given: three nodes on the list
        let world = setup(&[pid(2), pid(3)]);
        let x = pid(2);
        // when: I vote to exclude x with a reason
        let changed = world.apply(MemberAction::Exclude(x, Some("noisy link".into()))).await;
        // then: the record under the vote key carries it
        assert_eq!(changed, Ok(true));
        assert_eq!(world.note_of_mine(x).await, "noisy link");
    }

    #[tokio::test]
    async fn exclude_without_a_reason_stores_an_empty_note() {
        // given / when
        let world = setup(&[pid(2), pid(3)]);
        world.apply(MemberAction::Exclude(pid(2), None)).await.unwrap();
        // then
        assert_eq!(world.note_of_mine(pid(2)).await, "");
    }

    #[tokio::test]
    async fn repeated_exclude_with_another_reason_keeps_the_first_record() {
        // given: my vote with a reason
        let world = setup(&[pid(2), pid(3)]);
        let x = pid(2);
        world.apply(MemberAction::Exclude(x, Some("first".into()))).await.unwrap();
        let stored = world.mine(&world.shared, &exclude_key(&x)).await;
        // when: the same vote again with another reason
        let again = world.apply(MemberAction::Exclude(x, Some("second".into()))).await;
        // then: nothing is written, the note stays
        assert_eq!(again, Ok(false));
        assert_eq!(world.mine(&world.shared, &exclude_key(&x)).await, stored);
        assert_eq!(world.note_of_mine(x).await, "first");
    }

    #[tokio::test]
    async fn the_proposers_reason_shows_in_the_list_row() {
        // given: I propose to exclude x with a reason
        let world = setup(&[pid(2), pid(3)]);
        let x = pid(2);
        world.apply(MemberAction::Exclude(x, Some("noisy link".into()))).await.unwrap();
        // when / then: the row of x shows it
        let vote = world.row(x).vote.expect("the proposal must show");
        assert_eq!(vote.reason.as_deref(), Some("noisy link"));
    }
}

mod withdrawal {
    use super::*;

    #[tokio::test]
    async fn withdrawing_my_proposal_restores_the_plain_row() {
        // given: I propose to exclude x in a network of three
        let world = setup(&[pid(2), pid(3)]);
        let x = pid(2);
        world.apply(MemberAction::Exclude(x, Some("noisy link".into()))).await.unwrap();
        let row = world.row(x);
        assert_eq!(row.status, "member");
        assert!(row.vote.as_ref().is_some_and(|v| v.mine), "the proposal is mine");
        assert_eq!(row.denied_by_me, Some("voted_by_me"));
        // when: I take it back before anyone else voted
        let changed = world.apply(MemberAction::Unexclude(x)).await;
        // then: the row is the plain one again
        assert_eq!(changed, Ok(true));
        let row = world.row(x);
        assert_eq!(row.status, "member");
        assert!(row.vote.is_none(), "no vote must show");
        assert_eq!(row.denied_by_me, None);
        assert!(!row.blocked_by_me);
        assert!(world.reply_json()["members"].as_array().unwrap().iter().all(|r| r["vote"].is_null()));
        assert_eq!(world.verdict(x), Ok(()));
    }

    #[tokio::test]
    async fn withdrawing_after_a_second_vote_returns_the_node_with_the_other_vote_pending() {
        // given: I proposed, b confirmed, so x is excluded by two votes
        let world = setup(&[pid(2), pid(3)]);
        let (x, b) = (pid(2), pid(3));
        world.apply(MemberAction::Exclude(x, None)).await.unwrap();
        world.given_vote(b, x, LATE);
        world.refresh().await;
        assert_eq!(world.verdict(x), Err(Deny::Excluded));
        // when: I take my vote back
        let changed = world.apply(MemberAction::Unexclude(x)).await;
        // then: one vote is below the threshold, x returns and b's vote waits as a proposal
        assert_eq!(changed, Ok(true));
        assert_eq!(world.verdict(x), Ok(()));
        let vote = world.row(x).vote.expect("b's vote must show as a pending proposal");
        assert_eq!(vote.voters, vec![b.to_string()]);
        assert!(!vote.mine);
    }
}

impl World {
    /// The note of my live vote against `subject`.
    async fn note_of_mine(&self, subject: PeerId) -> String {
        let entry = self.mine(&self.shared, &exclude_key(&subject)).await.expect("my vote must exist");
        match parse_entry(&entry) {
            Parsed::Record { value, .. } => value.note,
            other => panic!("the vote must be a record, got {other:?}"),
        }
    }

    fn reply(&self) -> crate::web::MembersReply {
        let verdicts = self.access.view.verdicts().expect("the view must be published");
        members_reply(&verdicts, &self.access.members, &[], None)
    }

    fn reply_json(&self) -> serde_json::Value {
        serde_json::to_value(self.reply()).unwrap()
    }

    fn row(&self, peer: PeerId) -> MemberRow {
        self.reply().members.into_iter().find(|r| r.id_full == peer.to_string()).expect("the row must exist")
    }
}
