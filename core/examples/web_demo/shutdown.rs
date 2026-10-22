//! Waits for the stop signal and cleans up the demo's temporary settings directory after exit.

#[cfg(unix)]
pub(crate) async fn wait_for_sigterm() {
    use tokio::signal::unix::{signal, SignalKind};
    match signal(SignalKind::terminate()) {
        Ok(mut sig) => {
            sig.recv().await;
        }
        Err(_) => std::future::pending::<()>().await,
    }
}

#[cfg(not(unix))]
pub(crate) async fn wait_for_sigterm() {
    std::future::pending::<()>().await
}

pub(crate) fn cleanup_dir(dir: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let _ =
                    std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o644));
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}
