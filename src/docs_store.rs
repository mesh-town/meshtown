//! Directory path of a node's document store, next to the config file.
//!
//! What it does: provides the directory name `<config>/docs-store/` and the path to it from the config file.
//! What it does not do: it does not open the store — only `sync::replica::import` reads or moves it aside; it does not touch
//! the directory contents.
//! Links: `node_ctl::relaunch` calls `docs_store_path_beside` for the `NodeConfig::docs_store_dir` field.

use std::path::{Path, PathBuf};

/// Name of the store directory; it lies next to the config file.
pub const DOCS_STORE_DIR_NAME: &str = "docs-store";

/// `config` has no parent — `None`.
pub fn docs_store_path_beside(config: &Path) -> Option<PathBuf> {
    config.parent().map(|dir| dir.join(DOCS_STORE_DIR_NAME))
}
