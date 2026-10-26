//! Tests of the issue records under the rules of a Managed network: a record counts only if an admin wrote it
//! and it names an admin as the issuer (v2); a holder's copy carries the issuer's id; a v1 or an unreadable
//! record does not count. Also the byte layout of the stored value.
//!
//! assumed: this file is declared in `store_managed_tests.rs` as
//! `#[path = "store_managed_issue_tests.rs"] mod issue_records;` and uses its rig.

use super::*;
use crate::invite::store_admins::{issuer_of, ISSUE_V_ISSUER};

#[tokio::test]
async fn only_from_a_stranger_is_not_admin() {
    // given: the only issue record was written by a node that is not an admin
    let rig = setup().await;
    rig.given_issue_by(stranger(), NOW - 10);
    // when
    let got = rig.when_judged_among(&[admin()]).await;
    // then
    assert_eq!(got, IssueVerdict::NotAdmin);
}

#[tokio::test]
async fn from_an_admin_grants() {
    // given
    let rig = setup().await;
    rig.given_issue_by(admin(), NOW - 10);
    // when
    let got = rig.when_judged_among(&[admin()]).await;
    // then
    assert_eq!(got, IssueVerdict::Grant);
}

#[tokio::test]
async fn from_a_stranger_and_an_admin_grants() {
    // given: the stranger's record is ignored, the admin's one is enough
    let rig = setup().await;
    rig.given_issue_by(stranger(), NOW - 10);
    rig.given_issue_by(admin(), NOW - 10);
    // when
    let got = rig.when_judged_among(&[admin()]).await;
    // then
    assert_eq!(got, IssueVerdict::Grant);
}

#[tokio::test]
async fn fresh_one_of_a_stranger_does_not_save_an_expired_one_of_an_admin() {
    // given: the admin issued long ago, the stranger "issued" just now
    let rig = setup().await;
    rig.given_issue_by(admin(), NOW - TTL - 1);
    rig.given_issue_by(stranger(), NOW - 10);
    // when
    let got = rig.when_judged_among(&[admin()]).await;
    // then
    assert_eq!(got, IssueVerdict::Expired);
}

#[tokio::test]
async fn from_an_author_outside_an_empty_admin_set_is_not_admin() {
    // given
    let rig = setup().await;
    rig.given_issue_by(admin(), NOW - 10);
    // when: nobody is an admin at this node's view
    let got = rig.when_judged_among(&[]).await;
    // then
    assert_eq!(got, IssueVerdict::NotAdmin);
}

#[tokio::test]
async fn absent_is_unknown() {
    // given: no records at all
    let rig = setup().await;
    // when
    let got = rig.when_judged_among(&[admin()]).await;
    // then
    assert_eq!(got, IssueVerdict::Unknown);
}

mod holder_copy {
    use super::*;

    #[tokio::test]
    async fn copy_of_an_admin_issuer_grants() {
        // given: an admin holds a copy of the invite another admin issued
        let rig = setup().await;
        rig.given_copy_by(admin(), third(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin(), third()]).await;
        // then
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn copy_of_a_demoted_issuer_is_not_admin() {
        // given: the issuer wrote its own record and left a copy with an admin, then it was demoted
        let rig = setup().await;
        rig.given_issue_by(third(), NOW - 10);
        rig.given_copy_by(admin(), third(), NOW - 10);
        // when: only `admin` is an admin now
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(
            got,
            IssueVerdict::NotAdmin,
            "an invite of a demoted issuer must not be saved by a copy"
        );
    }

    #[tokio::test]
    async fn copy_kept_by_a_demoted_holder_does_not_cancel_the_issuer() {
        // given: an admin issued the invite and a holder, demoted since, kept a copy of it
        let rig = setup().await;
        rig.given_issue_by(admin(), NOW - 10);
        rig.given_copy_by(third(), admin(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn issuer_named_by_a_non_admin_writer_is_not_admin() {
        // given: a stranger wrote a record that names an admin as the issuer
        let rig = setup().await;
        rig.given_copy_by(stranger(), admin(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(
            got,
            IssueVerdict::NotAdmin,
            "the writer of the record must be an admin too"
        );
    }

    #[tokio::test]
    async fn used_mark_of_an_admin_beats_a_demoted_issuer() {
        // given: a copy for a demoted issuer and a redemption mark of an admin
        let rig = setup().await;
        rig.given_copy_by(admin(), third(), NOW - 10);
        rig.given_used_by(admin());
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::Used);
    }
}

mod old_record {
    use super::*;

    #[tokio::test]
    async fn v1_record_of_an_admin_is_not_admin() {
        // given: an admin wrote a v1 record (no issuer in it)
        let rig = setup().await;
        rig.given_v1_issue_by(admin(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::NotAdmin);
    }

    #[tokio::test]
    async fn unreadable_record_of_an_admin_is_not_admin() {
        // given: an admin's value that is not a record at all
        let rig = setup().await;
        rig.put(admin(), ISSUE_PREFIX, vec![0xff]);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::NotAdmin);
    }
}

mod mixed_formats {
    use super::*;

    #[tokio::test]
    async fn v1_of_a_holder_admin_does_not_hinder_v2_of_an_admin_author() {
        // given: a v1 record of an admin next to the v2 record of another admin
        let rig = setup().await;
        rig.given_v1_issue_by(admin(), NOW - 10);
        rig.given_issue_by(third(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin(), third()]).await;
        // then: only the v2 record counts, and it is enough
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn v1_of_an_admin_next_to_v2_of_a_stranger_is_not_admin() {
        // given: a v1 record of an admin and a v2 record of a node that is not an admin
        let rig = setup().await;
        rig.given_v1_issue_by(admin(), NOW - 10);
        rig.given_issue_by(stranger(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::NotAdmin);
    }

    #[tokio::test]
    async fn v1_of_a_non_admin_next_to_v2_of_an_admin_grants() {
        // given: a v1 record of a node that is not an admin and the v2 record of an admin
        let rig = setup().await;
        rig.given_v1_issue_by(stranger(), NOW - 10);
        rig.given_issue_by(admin(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then: the stranger's record is ignored
        assert_eq!(got, IssueVerdict::Grant);
    }

    #[tokio::test]
    async fn v1_of_a_non_admin_alone_is_not_admin() {
        // given
        let rig = setup().await;
        rig.given_v1_issue_by(stranger(), NOW - 10);
        // when
        let got = rig.when_judged_among(&[admin()]).await;
        // then
        assert_eq!(got, IssueVerdict::NotAdmin);
    }
}

mod value_layout {
    use super::*;

    #[test]
    fn plain_value_is_the_old_bytes() {
        // given
        let rec = issue_record(NOW, Some("Masha"));
        // when: no issuer
        let value = issue_value(&rec, None).unwrap();
        // then: exactly what a record was always stored as, and it names nobody
        assert_eq!(value, postcard::to_stdvec(&rec).unwrap());
        assert_eq!(issuer_of(&value), None);
    }

    #[test]
    fn v2_value_names_the_issuer() {
        // given
        let rec = issue_record(NOW, Some("Masha"));
        // when
        let value = issue_value(&rec, Some(admin())).unwrap();
        // then
        assert_eq!(issuer_of(&value), Some(admin()));
    }

    #[test]
    fn old_reader_takes_v2_as_before() {
        // given
        let rec = issue_record(NOW, Some("Masha"));
        let value = issue_value(&rec, Some(admin())).unwrap();
        // when: a reader that knows only the record, not the tail
        let read: IssueRecord = postcard::from_bytes(&value).unwrap();
        // then
        assert_eq!(read.made_unix, rec.made_unix);
        assert_eq!(read.for_whom, rec.for_whom);
        assert_eq!(read.v, ISSUE_V_ISSUER);
    }

    #[test]
    fn short_tail_has_no_issuer() {
        // given: a v2 value cut by its last byte
        let rec = issue_record(NOW, None);
        let value = issue_value(&rec, Some(admin())).unwrap();
        // when
        let got = issuer_of(&value[..value.len() - 1]);
        // then
        assert_eq!(got, None);
    }
}
