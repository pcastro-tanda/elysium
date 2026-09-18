//! Configuration loading failures.

use std::fmt;
use std::path::PathBuf;

/// Everything that can go wrong while loading a RuboCop configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// A configuration file could not be read.
    Io {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// RuboCop's `ConfigNotFoundError`.
    NotFound(PathBuf),
    /// The YAML could not be parsed.
    Yaml {
        /// The offending file.
        path: PathBuf,
        /// The parser message.
        message: String,
    },
    /// The document is valid YAML but not a mapping (`Malformed configuration`).
    Malformed(PathBuf),
    /// The file uses ERB templating, which this implementation does not run.
    ErbUnsupported(PathBuf),
    /// Obsolete, renamed, removed or extracted cops were configured.
    ObsoleteCop(String),
    /// `inherit_gem` named a gem that could not be located on disk.
    GemNotFound {
        /// The gem name from `inherit_gem`.
        gem: String,
        /// Version from `Gemfile.lock`, when one was found.
        version: Option<String>,
        /// Directories that were searched.
        searched: Vec<PathBuf>,
    },
    /// `inherit_gem: rubocop` is rejected by RuboCop itself.
    InheritFromRubocopGem,
    /// `inherit_from` pointed at an `http(s)` URL; remote configs are not
    /// fetched.
    RemoteUnsupported {
        /// The file holding the directive.
        path: PathBuf,
        /// The URL that was requested.
        url: String,
    },
    /// An `inherit_from` chain includes a file twice.
    CircularInheritance(PathBuf),
    /// A bare cop name matches cops in more than one department.
    AmbiguousCopName {
        /// The name as written in the configuration.
        name: String,
        /// The qualified candidates.
        candidates: Vec<String>,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            ConfigError::NotFound(path) => {
                write!(f, "Configuration file not found: {}", path.display())
            }
            ConfigError::Yaml { path, message } => {
                write!(f, "invalid YAML in {}: {message}", path.display())
            }
            ConfigError::Malformed(path) => {
                write!(f, "Malformed configuration in {}", path.display())
            }
            ConfigError::ErbUnsupported(path) => write!(
                f,
                "{} uses ERB (`<%`), which elysium does not evaluate; \
                 inline the generated values",
                path.display()
            ),
            ConfigError::ObsoleteCop(message) => write!(f, "{message}"),
            ConfigError::GemNotFound { gem, version, searched } => {
                write!(f, "Unable to find gem {gem}")?;
                if let Some(version) = version {
                    write!(f, " (version {version} from Gemfile.lock)")?;
                }
                write!(f, "; is the gem installed? Searched:")?;
                for dir in searched {
                    write!(f, "\n  {}", dir.display())?;
                }
                Ok(())
            }
            ConfigError::InheritFromRubocopGem => {
                write!(f, "can't inherit configuration from the rubocop gem")
            }
            ConfigError::RemoteUnsupported { path, url } => write!(
                f,
                "{} inherits from the remote configuration {url}; \
                 remote configs are not downloaded",
                path.display()
            ),
            ConfigError::CircularInheritance(path) => {
                write!(f, "circular inheritance detected at {}", path.display())
            }
            ConfigError::AmbiguousCopName { name, candidates } => write!(
                f,
                "Ambiguous cop name `{name}` needs department qualifier. Did you mean {}?",
                candidates.join(" or ")
            ),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
