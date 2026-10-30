//! The `config-path` command: prints the path to the settings file and starts nothing.

use crate::config::config_file::{self, ConfigSource};

/// The `config-path` command: prints the path to the config file and starts nothing.
pub(crate) fn config_path(config: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    match config_file::resolve_config_source(
        config.as_deref(),
        &config_file::current_env().as_path_env(),
    ) {
        ConfigSource::File { path, .. } => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigSource::Nowhere(why) => {
            anyhow::bail!("no place for the settings file: {why}")
        }
        // `config-path` doesn't know `--no-config`, so we can't get here; we don't go on silently.
        ConfigSource::Disabled => {
            anyhow::bail!("no place for the settings file: the settings file is disabled")
        }
    }
}
