//! Reading the parts of the shared document for one snapshot, each read under a timeout.

use std::collections::HashMap;
use std::time::Duration;

use crate::members::stale_peers;
use crate::node::node_state;
use crate::node::program_version;
use crate::node::RunningNode;
use crate::site_checks::geoip;
use crate::transport::net::PeerId;

use super::doc_cache::DocParts;

/// Timeout for each document read (`domains.list()`, `checks.check_views()`, `ports_doc.read_all()`, etc.).
const READ_TIMEOUT: Duration = Duration::from_millis(800);

/// Reads a document under `READ_TIMEOUT`: an error or timeout gives `empty` and `false`.
async fn read_or<T, E>(
    fut: impl std::future::Future<Output = Result<T, E>>,
    empty: T,
) -> (T, bool) {
    match tokio::time::timeout(READ_TIMEOUT, fut).await {
        Ok(Ok(v)) => (v, true),
        _ => (empty, false),
    }
}

/// Program versions of members from the shared document and the read-success flag (without the own version).
pub(super) async fn program_versions_of(node: &RunningNode) -> (HashMap<PeerId, String>, bool) {
    read_or(
        program_version::read_program_versions(node.domains.shared_replica().as_ref()),
        HashMap::new(),
    )
    .await
}

/// Time of each author's last write in the document and the read-success flag.
pub(super) async fn doc_seen_of(node: &RunningNode, now_unix: u64) -> (HashMap<PeerId, u64>, bool) {
    let r = node.domains.shared_replica();
    read_or(
        stale_peers::read_doc_seen(r.as_ref(), now_unix),
        HashMap::new(),
    )
    .await
}

/// Seven document reads under a timeout (`domains.list()`, `checks.check_views()`,
/// `node_state::read_exit_flags`, `geoip::read_countries`, program versions, `ports_doc.read_all()`,
/// `read_doc_seen`) and the "all succeeded" flag. Called only by `DocCache::read_doc_parts_cached`.
pub(super) async fn read_doc_parts(node: &RunningNode, now_unix: u64) -> (DocParts, bool) {
    let (list, list_ok) = read_or(node.domains.list(), Vec::new()).await;
    let (views, views_ok) = read_or(node.checks.check_views(), Vec::new()).await;
    let r = node.domains.shared_replica();
    let (exits, exits_ok) = read_or(node_state::read_exit_flags(r.as_ref()), HashMap::new()).await;
    let (countries, countries_ok) =
        read_or(geoip::read_countries(r.as_ref()), HashMap::new()).await;
    let (program_versions, versions_ok) = program_versions_of(node).await;
    let (port_views, ports_ok) = read_or(node.ports_doc.read_all(), Vec::new()).await;
    let (doc_seen, doc_seen_ok) = doc_seen_of(node, now_unix).await;
    let read_complete =
        list_ok && views_ok && exits_ok && countries_ok && ports_ok && versions_ok && doc_seen_ok;
    let parts = DocParts {
        list,
        views,
        exits,
        countries,
        program_versions,
        port_views,
        doc_seen,
    };
    (parts, read_complete)
}
