//! RuboCop-compatible configuration loading.
//!
//! The crate embeds RuboCop 1.82.1's `config/default.yml` and
//! `config/obsoletion.yml` (see `rubocop/LICENSE.txt`) and resolves user
//! configuration against them the way `RuboCop::ConfigLoader` does:
//! `inherit_from`, `inherit_gem`, `inherit_mode`, department-level switches,
//! `DisabledByDefault`/`EnabledByDefault`, `Enabled: pending` plus `NewCops`,
//! and `Exclude` absolutisation relative to the file that declares it.
//!
//! # Why `saphyr`
//!
//! Configuration is untyped nested hashes whose unknown keys must survive a
//! load/merge/`--show-cops` round trip, and `--show-cops` prints parameters in
//! document order. `saphyr` (the maintained yaml-rust2 successor) exposes a
//! generic document tree with insertion-ordered mappings and resolves anchors
//! and aliases, so no `serde` derive gymnastics are needed and no key order is
//! lost. `serde_yaml_ng` would have required a bespoke ordered-value type
//! anyway.
//!
//! Two deliberate differences from RuboCop:
//!
//! * ERB in configuration files is rejected instead of evaluated: there is no
//!   Ruby runtime ([`ConfigError::ErbUnsupported`]).
//! * `inherit_from` with an `http(s)` URL is rejected instead of downloaded
//!   ([`ConfigError::RemoteUnsupported`]).

mod defaults;
mod error;
mod file_matcher;
mod gems;
mod loader;
mod merge;
mod obsoletion;
mod paths;
mod resolved;
mod yaml;

pub use defaults::DEFAULT_YML;
pub use error::ConfigError;
pub use file_matcher::{FileMatcher, DEFAULT_EXCLUDE, DEFAULT_INCLUDE};
pub use loader::{ConfigLoader, DEFAULT_RUBY_VERSION, DOTFILE, XDG_CONFIG};
pub use resolved::{AllCops, CopConfig, LoadedConfig, NewCops};
pub use yaml::{Mapping, YamlValue};
