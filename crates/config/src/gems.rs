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

/// Outcome of looking up an extension gem's own `config/default.yml`.
pub(crate) enum ExtensionDefaults {
    /// The gem was located but ships no `config/default.yml` of its own.
    NotShipped,
    /// The gem's own default configuration file, ready to be read and merged.
    Found(PathBuf),
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
        let version = locked_version(project_root, gem, self.bundle_gemfile_lockfile().as_deref());
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

    /// Locates `<gem>/config/default.yml` the way `inherit_gem` locates a
    /// gem's config, but treats a missing *file* as `NotShipped` rather than
    /// an error: only a gem that cannot be found at all is an error. This is
    /// how RuboCop injects a `require:`/`plugins:` gem's own defaults below
    /// the embedded `config/default.yml`
    /// (`Plugin::ConfigurationIntegrator`, `ConfigLoader.inject_defaults!`).
    pub(crate) fn extension_defaults(
        &self,
        project_root: &Path,
        gem: &str,
    ) -> Result<ExtensionDefaults, ConfigError> {
        let path = self.config_path(project_root, gem, "config/default.yml")?;
        Ok(if path.is_file() {
            ExtensionDefaults::Found(path)
        } else {
            ExtensionDefaults::NotShipped
        })
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

    /// The lockfile Bundler would resolve gem versions against instead of
    /// searching upward from the linted project: `Bundler.default_gemfile`
    /// prefers `$BUNDLE_GEMFILE` over discovering a `Gemfile` from the
    /// working directory, and `rubocop`/`Bundler.load.specs` then resolve
    /// gem versions from *that* Gemfile's lockfile. A linted project can
    /// ship its own unrelated `Gemfile.lock` (e.g. an app that itself
    /// depends on `rubocop-rails` at one version, linted under a harness
    /// that installs the plugin gems from a separate `BUNDLE_GEMFILE` at
    /// another) that must not shadow the lockfile actually governing the
    /// installed gems. Consulted only when `use_environment` is set, so
    /// tests stay hermetic.
    fn bundle_gemfile_lockfile(&self) -> Option<PathBuf> {
        if !self.use_environment {
            return None;
        }
        let gemfile = std::env::var_os("BUNDLE_GEMFILE").map(PathBuf::from)?;
        gemfile.is_file().then(|| bundle_lockfile_path(&gemfile))
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

/// Reads `<gem> (<version>)`, preferring `preferred_lockfile` (Bundler's
/// resolved lockfile for `$BUNDLE_GEMFILE`, when the caller has one) over
/// searching upward from `project_root`, the way `Bundler.load.specs`
/// resolves gem versions from whichever Gemfile governs the running
/// process rather than the directory being inspected. A `preferred_lockfile`
/// that exists but doesn't mention `gem` is authoritative and is not a
/// reason to fall back: that mirrors a real `bundle exec rubocop` raising
/// rather than silently consulting an unrelated lockfile.
pub(crate) fn locked_version(
    project_root: &Path,
    gem: &str,
    preferred_lockfile: Option<&Path>,
) -> Option<String> {
    match preferred_lockfile {
        Some(path) if path.is_file() => read_locked_version(path, gem),
        _ => read_locked_version(&find_lockfile(project_root)?, gem),
    }
}

fn read_locked_version(lockfile: &Path, gem: &str) -> Option<String> {
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

/// `Bundler::SharedHelpers.default_lockfile`: a `gems.rb` Gemfile locks to
/// `gems.locked` alongside it; anything else (almost always `Gemfile`)
/// locks to `<name>.lock`.
fn bundle_lockfile_path(gemfile: &Path) -> PathBuf {
    if gemfile.file_name() == Some(OsStr::new("gems.rb")) {
        gemfile.with_file_name("gems.locked")
    } else {
        let mut with_suffix = gemfile.as_os_str().to_os_string();
        with_suffix.push(".lock");
        PathBuf::from(with_suffix)
    }
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
    fn preferred_lockfile_overrides_the_projects_own_lockfile() {
        // A harness that installs plugin gems from a separate
        // `BUNDLE_GEMFILE` must resolve versions against *that* lockfile,
        // not a same-named `Gemfile.lock` the linted project happens to
        // ship for its own unrelated purposes (e.g. an app that also
        // depends on `styles` itself, at another version).
        let dir = tempfile::tempdir().unwrap();
        let gems = dir.path().join("gems");
        for name in ["styles-1.2.0", "styles-1.10.0"] {
            std::fs::create_dir_all(gems.join(name)).unwrap();
        }
        std::fs::write(
            dir.path().join("Gemfile.lock"),
            "GEM\n  specs:\n    styles (1.2.0)\n\nDEPENDENCIES\n  styles\n",
        )
        .unwrap();
        let bundle_dir = tempfile::tempdir().unwrap();
        let bundle_lockfile = bundle_dir.path().join("Gemfile.lock");
        std::fs::write(
            &bundle_lockfile,
            "GEM\n  specs:\n    styles (1.10.0)\n\nDEPENDENCIES\n  styles\n",
        )
        .unwrap();
        let version = locked_version(dir.path(), "styles", Some(&bundle_lockfile));
        assert_eq!(version.as_deref(), Some("1.10.0"));
    }

    #[test]
    fn bundle_lockfile_path_follows_bundler_naming() {
        assert_eq!(bundle_lockfile_path(Path::new("/app/Gemfile")), Path::new("/app/Gemfile.lock"));
        assert_eq!(
            bundle_lockfile_path(Path::new("/app/ci/rubocop.gemfile")),
            Path::new("/app/ci/rubocop.gemfile.lock")
        );
        assert_eq!(bundle_lockfile_path(Path::new("/app/gems.rb")), Path::new("/app/gems.locked"));
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
