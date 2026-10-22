use super::*;
use crate::config::config_file::overrides::CliOverrides;
use crate::config::config_file::path::{ConfigSource, PathRule};
use crate::config::config_file::test_support::scratch;
use crate::config::config_file::FILE_NAME;

const NOW: u64 = 1_790_278_867;

#[test]
fn created_file_is_taken_by_prepare_like_run_file() {
    let dir = scratch("fresh-prepare");
    let file = dir.join("sub").join(FILE_NAME);
    let fresh = create_fresh(&file, NOW, |_| {}).expect("create_fresh");
    assert_eq!(fresh.saved.expect("write"), 1);
    let source = ConfigSource::File {
        path: file.clone(),
        may_create: true,
        rule: PathRule::Explicit,
    };
    let run = crate::config::config_file::prepare(&source, &CliOverrides::default(), false, NOW)
        .expect("prepare");
    assert!(run.file_report.persistent_identity);
    assert_eq!(
        run.file_report.state,
        crate::config::config_file::FileState::Ok
    );
    // The secret was not recreated: the revision stayed the first.
    assert_eq!(run.file_report.rev, 1);
    assert!(run.effective_settings.net.network_key.is_none());
    assert!(run.effective_settings.net.node_name.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}
