//! The target project's locked gem versions, RuboCop's
//! `Config#gem_versions_in_target` (`lib/rubocop/config.rb`) over
//! `RuboCop::Lockfile#gem_versions` (`lib/rubocop/lockfile.rb`).
//!
//! RuboCop asks `Bundler::LockfileParser` for the lockfile's specs, so every
//! gem the bundle resolved counts, transitive ones included. A lockfile
//! holds them as four-space-indented `name (version)` lines under the
//! `specs:` header of each `GEM`, `GIT`, `PATH` and `PLUGIN SOURCE` section;
//! a platform-specific entry reads `name (version-platform)` and the version
//! is the part before the first `-`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths;

/// Locked gem versions, keyed by gem name; the versions are the lockfile's
/// own text (`"3.1.0"`), compared by the rules.
pub type GemVersions = BTreeMap<String, String>;

/// `Config#bundler_lock_file_path`: `Gemfile.lock`, else `gems.locked`,
/// each searched from `base_dir` upward (`find_file_upwards`), nearest first.
pub(crate) fn find_bundler_lockfile(base_dir: &Path) -> Option<PathBuf> {
    ["Gemfile.lock", "gems.locked"].iter().find_map(|name| {
        paths::ancestors(base_dir, None)
            .into_iter()
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// `Lockfile#gem_versions` of the lockfile at `path`: empty when the file
/// cannot be read, as RuboCop's parser rescues `Bundler::BundlerError`.
pub(crate) fn read_gem_versions(path: &Path) -> GemVersions {
    std::fs::read(path)
        .map(|bytes| parse_gem_versions(&String::from_utf8_lossy(&bytes)))
        .unwrap_or_default()
}

/// `Bundler::LockfileParser#specs` as a name-to-version map; a gem listed
/// twice keeps its last entry, as `to_h` does.
pub(crate) fn parse_gem_versions(text: &str) -> GemVersions {
    let mut versions = GemVersions::new();
    let mut in_source = false;
    let mut in_specs = false;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            in_source = false;
            in_specs = false;
        } else if !line.starts_with(' ') {
            in_source = matches!(line, "GEM" | "GIT" | "PATH" | "PLUGIN SOURCE");
            in_specs = false;
        } else if in_source && line == "  specs:" {
            in_specs = true;
        } else if in_specs && line.starts_with("    ") && !line.starts_with("     ") {
            if let Some((name, version)) = spec_entry(&line[4..]) {
                versions.insert(name.to_string(), version.to_string());
            }
        }
    }
    versions
}

/// `name (version)` or `name (version-platform)`.
fn spec_entry(entry: &str) -> Option<(&str, &str)> {
    let (name, rest) = entry.split_once(" (")?;
    let inner = rest.strip_suffix(')')?;
    let version = inner.split('-').next().unwrap_or(inner);
    (!name.is_empty() && !version.is_empty()).then_some((name, version))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = "\
GIT
  remote: https://example.test/x.git
  revision: abc
  specs:
    forked (2.0.1)
      rack (>= 1)

GEM
  remote: https://rubygems.org/
  specs:
    nokogiri (1.16.0-arm64-darwin)
    rack (3.1.0)
    railties (7.1.3.2)
      actionpack (= 7.1.3.2)

PLATFORMS
  arm64-darwin

DEPENDENCIES
  rack

BUNDLED WITH
   2.5.0
";

    #[test]
    fn reads_specs_but_not_their_dependencies() {
        let versions = parse_gem_versions(LOCK);
        let got: Vec<_> = versions.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        assert_eq!(
            got,
            [
                ("forked", "2.0.1"),
                ("nokogiri", "1.16.0"),
                ("rack", "3.1.0"),
                ("railties", "7.1.3.2"),
            ]
        );
    }

    #[test]
    fn loaded_config_finds_the_lockfile_above_its_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Gemfile.lock"), LOCK).unwrap();
        let nested = dir.path().join("app");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join(".rubocop.yml"), "AllCops:\n  NewCops: enable\n").unwrap();
        let loader = crate::ConfigLoader::new().with_cwd(&nested).with_gem_lookup(false);
        let config = loader.load(Some(&nested.join(".rubocop.yml"))).unwrap();
        assert_eq!(config.gem_versions().unwrap().get("rack").map(String::as_str), Some("3.1.0"));
        assert!(loader.load(None).unwrap().gem_versions().is_none());
    }
}
