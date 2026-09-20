//! Shared configuration resolution for the `check` and `config` subcommands.

use std::path::{Path, PathBuf};

use config::{ConfigError, ConfigLoader, LoadedConfig};

/// Resolves and loads the effective configuration rooted at `cwd`.
///
/// `config` (the `--config PATH` flag) wins if given; otherwise `no_config`
/// (`--no-config`) forces the bundled defaults with no file; otherwise the
/// loader searches upward from `cwd` for a `.rubocop.yml`.
pub fn load_config(
    config: Option<&Path>,
    no_config: bool,
    cwd: &Path,
) -> Result<LoadedConfig, ConfigError> {
    let loader = ConfigLoader::new().with_cwd(cwd.to_path_buf());
    let path: Option<PathBuf> = if let Some(path) = config {
        Some(path.to_path_buf())
    } else if no_config {
        None
    } else {
        loader.find_config_file(cwd)
    };
    loader.load(path.as_deref())
}
