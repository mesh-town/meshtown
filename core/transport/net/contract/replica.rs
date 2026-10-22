//! Contract suite of the replica: the `replica_contract!` macro; the cases are plain functions in `replica_cases`
//! (one node) and `replica_sync_cases` (two nodes).
//! Plugging in an adapter: `replica_contract!(async_fn_without_arguments_returning_SyncPair);`
//! — the harness conventions are described at `SyncPair`.

macro_rules! replica_contract {
    ($harness:path) => {
        mod replica {
            use super::*;
            use $crate::transport::net::contract::replica_cases as one;
            use $crate::transport::net::contract::replica_sync_cases as two;

            mod writing {
                use super::*;

                #[tokio::test]
                async fn set_get_roundtrip() {
                    one::set_get_roundtrip($harness().await).await
                }

                #[tokio::test]
                async fn overwrite_keeps_one_entry() {
                    one::overwrite_keeps_one_entry($harness().await).await
                }

                #[tokio::test]
                async fn empty_value_rejected() {
                    one::empty_value_rejected($harness().await).await
                }

                #[tokio::test]
                async fn oversize_value_rejected() {
                    one::oversize_value_rejected($harness().await).await
                }

                #[tokio::test]
                async fn ts_within_minute() {
                    one::ts_within_minute($harness().await).await
                }
            }

            mod deleting {
                use super::*;

                #[tokio::test]
                async fn del_own_hides_and_leaves_tombstone() {
                    one::del_own_hides_and_leaves_tombstone($harness().await).await
                }

                #[tokio::test]
                async fn del_missing_key_is_ok() {
                    one::del_missing_key_is_ok($harness().await).await
                }

                #[tokio::test]
                async fn tombstone_emits_local_change() {
                    one::tombstone_emits_local_change($harness().await).await
                }
            }

            mod scanning {
                use super::*;

                #[tokio::test]
                async fn scan_order_author_then_key() {
                    two::scan_order_author_then_key($harness().await).await
                }

                #[tokio::test]
                async fn local_change_on_write_not_on_read() {
                    one::local_change_on_write_not_on_read($harness().await).await
                }
            }

            mod names {
                use super::*;

                #[tokio::test]
                async fn domains_refuses_sync() {
                    one::domains_refuses_sync($harness().await).await
                }

                #[tokio::test]
                async fn unknown_replica_name_rejected() {
                    one::unknown_replica_name_rejected($harness().await).await
                }

                #[tokio::test]
                async fn same_name_same_replica() {
                    one::same_name_same_replica($harness().await).await
                }

                #[tokio::test]
                async fn domains_stays_private() {
                    two::domains_stays_private($harness().await).await
                }
            }

            mod syncing {
                use super::*;

                #[tokio::test]
                async fn sync_delivers_and_reports_synced() {
                    two::sync_delivers_and_reports_synced($harness().await).await
                }

                #[tokio::test]
                async fn repeat_sync_no_duplicates() {
                    two::repeat_sync_no_duplicates($harness().await).await
                }

                #[tokio::test]
                async fn live_push_after_sync_emits_remote_change() {
                    two::live_push_after_sync_emits_remote_change($harness().await).await
                }

                #[tokio::test]
                async fn tombstone_travels() {
                    two::tombstone_travels($harness().await).await
                }
            }

            mod latest {
                use super::*;

                #[tokio::test]
                async fn latest_picks_newest_across_authors() {
                    two::latest_picks_newest_across_authors($harness().await).await
                }

                #[tokio::test]
                async fn latest_tie_breaks_by_author() {
                    two::latest_tie_breaks_by_author($harness().await).await
                }

                #[tokio::test]
                async fn same_key_two_authors_two_entries() {
                    two::same_key_two_authors_two_entries($harness().await).await
                }
            }
        }
    };
}

pub(crate) use replica_contract;
