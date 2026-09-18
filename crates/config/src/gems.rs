//! Locating installed gem directories for `inherit_gem`, without Ruby.
//!
//! RuboCop asks Bundler (`Bundler.load.specs`) and then `RubyGems`
//! (`Gem::Specification.find_by_name`). Neither is available here, so the
//! version is read from `Gemfile.lock` and the gem directory is looked up in
//! the usual install roots.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::ConfigError;

/// Where to look for `<gem>-<version>/` directories.
#[derive(Debug, Clone, Default)]
pub(crate) struct GemSearch {
    /// Explicit roots (tests, or a caller-provided `GEM_HOME`).
    pub extra_roots: Vec<PathBuf>,
    /// Whether the process environment and the usual per-user install roots
    /// may be consulted.
    pub use_environment: bool,
}

impl GemSearch {
    /// Resolves `<gem_dir>/<relative_config_path>` the way
    /// `ConfigLoaderResolver#gem_config_path` does.
    pub(crate) fn config_path(
        &self,
        project_root: &Path,
        gem: &str,
        relative_config_path: &str,
    ) -> Result<PathBuf, ConfigError> {
        if gem == "rubocop" {
            return Err(ConfigError::InheritFromRubocopGem);
        }
        let version = locked_version(project_root, gem);
        let roots = self.gem_roots(project_root);
        let mut best: Option<(Vec<u64>, PathBuf)> = None;
        for root in &roots {
            let Ok(entries) = std::fs::read_dir(root) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                let Some(found) = name.strip_prefix(gem).and_then(|rest| rest.strip_prefix('-'))
                else {
                    continue;
                };
                if let Some(want) = &version {
                    if found != want {
                        continue;
                    }
                }
                let parsed = version_segments(found);
                if best.as_ref().is_none_or(|(b, _)| *b < parsed) {
                    best = Some((parsed, entry.path()));
                }
            }
        }
        match best {
            Some((_, dir)) => Ok(dir.join(relative_config_path)),
            None => {
                Err(ConfigError::GemNotFound { gem: gem.to_string(), version, searched: roots })
            }
        }
    }

    /// Directories that directly contain `<gem>-<version>` entries.
    fn gem_roots(&self, project_root: &Path) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::new();
        let push = |dir: PathBuf, roots: &mut Vec<PathBuf>| {
            if !roots.contains(&dir) {
                roots.push(dir);
            }
        };
        for root in &self.extra_roots {
            push(root.clone(), &mut roots);
            push(root.join("gems"), &mut roots);
            for dir in expand_star(&root.join("ruby").join("*").join("gems")) {
                push(dir, &mut roots);
            }
        }
        for dir in expand_star(&project_root.join("vendor/bundle/ruby/*/gems")) {
            push(dir, &mut roots);
        }
        push(project_root.join("vendor/bundle/gems"), &mut roots);
        if !self.use_environment {
            return roots;
        }
        for var in ["GEM_HOME", "GEM_PATH", "BUNDLE_PATH"] {
            let Ok(value) = std::env::var(var) else { continue };
            for part in value.split(':').filter(|p| !p.is_empty()) {
                let base = PathBuf::from(part);
                push(base.join("gems"), &mut roots);
                push(base.clone(), &mut roots);
                for dir in expand_star(&base.join("ruby").join("*").join("gems")) {
                    push(dir, &mut roots);
                }
            }
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            let patterns = [
                ".gem/ruby/*/gems",
                ".rbenv/versions/*/lib/ruby/gems/*/gems",
                ".rvm/gems/*/gems",
                ".asdf/installs/ruby/*/lib/ruby/gems/*/gems",
            ];
            for pattern in patterns {
                for dir in expand_star(&home.join(pattern)) {
                    push(dir, &mut roots);
                }
            }
        }
        for pattern in ["/usr/local/lib/ruby/gems/*/gems", "/opt/homebrew/lib/ruby/gems/*/gems"] {
            for dir in expand_star(Path::new(pattern)) {
                push(dir, &mut roots);
            }
        }
        roots
    }
}

/// Expands `*` path components by reading directories; no other glob syntax is
/// supported because none of the search patterns needs it.
fn expand_star(pattern: &Path) -> Vec<PathBuf> {
    let mut current: Vec<PathBuf> = Vec::new();
    let mut first = true;
    for component in pattern.components() {
        let part = component.as_os_str();
        if first {
            current.push(PathBuf::from(part));
            first = false;
            continue;
        }
        if part == OsStr::new("*") {
            let mut next = Vec::new();
            for base in &current {
                let Ok(entries) = std::fs::read_dir(base) else { continue };
                for entry in entries.flatten() {
                    if entry.file_type().is_ok_and(|t| t.is_dir()) {
                        next.push(entry.path());
                    }
                }
            }
            next.sort();
            current = next;
        } else {
            for base in &mut current {
                base.push(part);
            }
        }
        if current.is_empty() {
            return current;
        }
    }
    current
}

/// Reads `<gem> (<version>)` from the nearest `Gemfile.lock`/`gems.locked`.
pub(crate) fn locked_version(project_root: &Path, gem: &str) -> Option<String> {
    let lockfile = find_lockfile(project_root)?;
    let contents = std::fs::read_to_string(lockfile).ok()?;
    let needle = format!("{gem} (");
    for line in contents.lines() {
        let trimmed = line.trim_start();
        // Dependencies are indented four spaces under `specs:`; the
        // `DEPENDENCIES` section lists them with two.
        if line.len() - trimmed.len() != 4 {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(&needle) {
            if let Some(version) = rest.strip_suffix(')') {
                return Some(version.to_string());
            }
        }
    }
    None
}

/// The nearest lock file at or above `dir`.
pub(crate) fn find_lockfile(dir: &Path) -> Option<PathBuf> {
    let mut current = Some(dir);
    while let Some(d) = current {
        for name in ["Gemfile.lock", "gems.locked"] {
            let candidate = d.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        current = d.parent();
    }
    None
}

/// Numeric segments of a gem version, for "newest wins" comparisons.
fn version_segments(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_version_wins_when_the_lockfile_is_silent() {
        let dir = tempfile::tempdir().unwrap();
        let gems = dir.path().join("gems");
        for name in ["styles-1.2.0", "styles-1.10.0", "styles-0.9.0"] {
            std::fs::create_dir_all(gems.join(name)).unwrap();
        }
        let search =
            GemSearch { extra_roots: vec![dir.path().to_path_buf()], ..Default::default() };
        let path = search.config_path(dir.path(), "styles", "config/default.yml").unwrap();
        assert_eq!(path, gems.join("styles-1.10.0/config/default.yml"));
    }

    #[test]
    fn lockfile_version_is_preferred() {
        let dir = tempfile::tempdir().unwrap();
        let gems = dir.path().join("gems");
        for name in ["styles-1.2.0", "styles-1.10.0"] {
            std::fs::create_dir_all(gems.join(name)).unwrap();
        }
        std::fs::write(
            dir.path().join("Gemfile.lock"),
            "GEM\n  specs:\n    styles (1.2.0)\n      rubocop (~> 1.0)\n\nDEPENDENCIES\n  styles\n",
        )
        .unwrap();
        let search =
            GemSearch { extra_roots: vec![dir.path().to_path_buf()], ..Default::default() };
        let path = search.config_path(dir.path(), "styles", "config/default.yml").unwrap();
        assert_eq!(path, gems.join("styles-1.2.0/config/default.yml"));
    }

    #[test]
    fn missing_gem_reports_searched_paths() {
        let dir = tempfile::tempdir().unwrap();
        let search =
            GemSearch { extra_roots: vec![dir.path().to_path_buf()], ..Default::default() };
        let err = search.config_path(dir.path(), "styles", "config/default.yml").unwrap_err();
        let message = err.to_string();
        assert!(message.starts_with("Unable to find gem styles"), "{message}");
        assert!(message.contains(&dir.path().join("gems").display().to_string()), "{message}");
    }

    #[test]
    fn rubocop_gem_is_rejected() {
        // spec/rubocop/config_loader_spec.rb:1251 "when a file inherits from the rubocop gem"
        let search = GemSearch::default();
        assert!(matches!(
            search.config_path(Path::new("/"), "rubocop", "config/default.yml"),
            Err(ConfigError::InheritFromRubocopGem)
        ));
    }

    #[test]
    fn bundler_vendor_layout_is_searched() {
        let dir = tempfile::tempdir().unwrap();
        let gems = dir.path().join("vendor/bundle/ruby/3.4.0/gems/styles-2.0.0");
        std::fs::create_dir_all(&gems).unwrap();
        let search = GemSearch::default();
        let path = search.config_path(dir.path(), "styles", "rubocop.yml").unwrap();
        assert_eq!(path, gems.join("rubocop.yml"));
    }
}
