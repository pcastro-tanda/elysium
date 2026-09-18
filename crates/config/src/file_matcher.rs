use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};

/// RuboCop's `AllCops/Include` defaults (rubocop 1.82).
pub const DEFAULT_INCLUDE: &[&str] = &[
    "**/*.rb",
    "**/*.arb",
    "**/*.axlsx",
    "**/*.builder",
    "**/*.fcgi",
    "**/*.gemfile",
    "**/*.gemspec",
    "**/*.god",
    "**/*.jb",
    "**/*.jbuilder",
    "**/*.mspec",
    "**/*.opal",
    "**/*.pluginspec",
    "**/*.podspec",
    "**/*.rabl",
    "**/*.rake",
    "**/*.rbuild",
    "**/*.rbw",
    "**/*.rbx",
    "**/*.ru",
    "**/*.ruby",
    "**/*.schema",
    "**/*.spec",
    "**/*.thor",
    "**/*.watchr",
    "**/.irbrc",
    "**/.pryrc",
    "**/.simplecov",
    "**/buildfile",
    "**/Appraisals",
    "**/Berksfile",
    "**/Brewfile",
    "**/Buildfile",
    "**/Capfile",
    "**/Cheffile",
    "**/Dangerfile",
    "**/Deliverfile",
    "**/Fastfile",
    "**/*Fastfile",
    "**/Gemfile",
    "**/Guardfile",
    "**/Jarfile",
    "**/Mavenfile",
    "**/Podfile",
    "**/Puppetfile",
    "**/Rakefile",
    "**/rakefile",
    "**/Schemafile",
    "**/Snapfile",
    "**/Steepfile",
    "**/Thorfile",
    "**/Vagabondfile",
    "**/Vagrantfile",
];

/// RuboCop's `AllCops/Exclude` defaults (rubocop 1.82).
pub const DEFAULT_EXCLUDE: &[&str] = &["node_modules/**/*", "tmp/**/*", "vendor/**/*", ".git/**/*"];

/// Decides which files under a project root are lint targets.
///
/// Patterns follow RuboCop semantics: relative to the project root, `*` does
/// not cross `/`, `**/` matches zero or more directories.
#[derive(Debug, Clone)]
pub struct FileMatcher {
    include: GlobSet,
    exclude: GlobSet,
}

impl FileMatcher {
    /// Builds a matcher from include and exclude pattern lists.
    pub fn new(include: &[&str], exclude: &[&str]) -> Result<Self, globset::Error> {
        Ok(Self { include: build(include)?, exclude: build(exclude)? })
    }

    /// RuboCop's defaults.
    pub fn rubocop_defaults() -> Self {
        Self::new(DEFAULT_INCLUDE, DEFAULT_EXCLUDE).expect("default patterns are valid")
    }

    /// True when `relative` (a path relative to the project root) matches an
    /// include pattern.
    pub fn is_included(&self, relative: &Path) -> bool {
        self.include.is_match(relative)
    }

    /// True when `relative` matches an exclude pattern.
    pub fn is_excluded(&self, relative: &Path) -> bool {
        self.exclude.is_match(relative)
    }

    /// Included and not excluded.
    pub fn is_target(&self, relative: &Path) -> bool {
        self.is_included(relative) && !self.is_excluded(relative)
    }
}

fn build(patterns: &[&str]) -> Result<GlobSet, globset::Error> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(p)?);
    }
    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_rubocop_expectations() {
        let m = FileMatcher::rubocop_defaults();
        assert!(m.is_target(Path::new("app/models/user.rb")));
        assert!(m.is_target(Path::new("user.rb")));
        assert!(m.is_target(Path::new("Gemfile")));
        assert!(m.is_target(Path::new("lib/tasks/db.rake")));
        assert!(m.is_target(Path::new("config.ru")));
        assert!(m.is_target(Path::new("fastlane/Fastfile")));
        assert!(!m.is_target(Path::new("vendor/bundle/gems/x/lib/x.rb")));
        assert!(!m.is_target(Path::new("node_modules/x/y.rb")));
        assert!(!m.is_target(Path::new("tmp/cache/x.rb")));
        assert!(!m.is_target(Path::new("app/assets/app.js")));
        assert!(!m.is_target(Path::new("README.md")));
        // Only the top-level vendor dir is excluded by default.
        assert!(m.is_target(Path::new("app/vendor/x.rb")));
    }
}
