//! `config path`: prints the path to the settings file and starts nothing.

use crate::cli::args::Globals;
use crate::config::config_file::{self, ConfigSource};

pub(super) fn run(g: &Globals) -> anyhow::Result<()> {
    g.allow("config path", &[])?;
    match config_file::resolve_config_source(
        g.config.as_deref(),
        &config_file::current_env().as_path_env(),
    ) {
        ConfigSource::File { path, .. } => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigSource::Nowhere(why) => {
            anyhow::bail!("no place for the settings file: {why}")
        }
        // `config path` has no `--no-config`, so this is not reached; it does not go on silently.
        ConfigSource::Disabled => {
            anyhow::bail!("no place for the settings file: the settings file is disabled")
        }
    }
}
