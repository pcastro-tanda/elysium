use std::path::{Path, PathBuf};

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};

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
/// Patterns follow RuboCop semantics: `*` does not cross `/`
/// (`File::FNM_PATHNAME`), `**/` matches zero or more directories, and a
/// pattern is matched against both the path relative to the directory holding
/// the configuration file and the absolute path. RuboCop rewrites every
/// `Exclude` entry to an absolute path when a config file is loaded
/// (`Config#make_excludes_absolute`), so both forms occur in practice.
#[derive(Debug, Clone)]
pub struct FileMatcher {
    root: PathBuf,
    include: Clusivity,
    exclude: Clusivity,
}

/// One side (`Include` or `Exclude`) of a matcher.
#[derive(Debug, Clone)]
struct Clusivity {
    relative: GlobSet,
    absolute: GlobSet,
    has_absolute: bool,
    /// True when the configuration does not set this key at all, in which case
    /// RuboCop's `file_name_matches_any?` returns its default answer.
    unset: bool,
}

impl Clusivity {
    fn build(patterns: Option<&[String]>) -> Result<Self, globset::Error> {
        let Some(patterns) = patterns else {
            return Ok(Self {
                relative: GlobSet::empty(),
                absolute: GlobSet::empty(),
                has_absolute: false,
                unset: true,
            });
        };
        let mut relative = GlobSetBuilder::new();
        let mut absolute = GlobSetBuilder::new();
        let mut has_absolute = false;
        for pattern in patterns {
            let Ok(glob) = compile(pattern) else {
                // RuboCop's `File.fnmatch?` treats a malformed pattern as a
                // literal that simply never matches a real path.
                continue;
            };
            if Path::new(pattern).is_absolute() {
                has_absolute = true;
                absolute.add(glob);
            } else {
                relative.add(glob);
            }
        }
        Ok(Self {
            relative: relative.build()?,
            absolute: absolute.build()?,
            has_absolute,
            unset: false,
        })
    }

    fn is_match(&self, root: &Path, relative: &Path, default: bool) -> bool {
        if self.unset {
            return default;
        }
        if self.relative.is_match(relative) {
            return true;
        }
        self.has_absolute && self.absolute.is_match(root.join(relative))
    }
}

impl FileMatcher {
    /// Builds a matcher from include and exclude pattern lists, rooted at the
    /// current directory.
    pub fn new(include: &[&str], exclude: &[&str]) -> Result<Self, globset::Error> {
        let owned = |p: &[&str]| p.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        Self::rooted(PathBuf::new(), Some(&owned(include)), Some(&owned(exclude)))
    }

    /// Builds a matcher for `root`, where `None` means the configuration does
    /// not set that key: an unset `Include` includes everything and an unset
    /// `Exclude` excludes nothing.
    pub fn rooted(
        root: impl Into<PathBuf>,
        include: Option<&[String]>,
        exclude: Option<&[String]>,
    ) -> Result<Self, globset::Error> {
        Ok(Self {
            root: root.into(),
            include: Clusivity::build(include)?,
            exclude: Clusivity::build(exclude)?,
        })
    }

    /// RuboCop's defaults.
    pub fn rubocop_defaults() -> Self {
        Self::new(DEFAULT_INCLUDE, DEFAULT_EXCLUDE).expect("default patterns are valid")
    }

    /// The directory relative paths are resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// True when `relative` (a path relative to the project root) matches an
    /// include pattern.
    pub fn is_included(&self, relative: &Path) -> bool {
        self.include.is_match(&self.root, relative, true)
    }

    /// True when `relative` matches an exclude pattern.
    pub fn is_excluded(&self, relative: &Path) -> bool {
        self.exclude.is_match(&self.root, relative, false)
    }

    /// Included and not excluded.
    pub fn is_target(&self, relative: &Path) -> bool {
        self.is_included(relative) && !self.is_excluded(relative)
    }
}

fn compile(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern).literal_separator(true).build()
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

    #[test]
    fn absolute_exclude_patterns_match_via_root() {
        let m = FileMatcher::rooted(
            "/project",
            Some(&["**/*.rb".to_string()]),
            Some(&["/project/db/schema.rb".to_string(), "spec/**/*".to_string()]),
        )
        .unwrap();
        assert!(m.is_target(Path::new("app/user.rb")));
        assert!(!m.is_target(Path::new("db/schema.rb")));
        assert!(!m.is_target(Path::new("spec/models/user_spec.rb")));
    }

    #[test]
    fn unset_clusivity_uses_rubocop_defaults() {
        let m = FileMatcher::rooted("/project", None, Some(&["lib/**/*".to_string()])).unwrap();
        // An unset `Include` includes everything; an unset `Exclude` excludes nothing.
        assert!(m.is_target(Path::new("anything.txt")));
        assert!(!m.is_target(Path::new("lib/foo.rb")));
        let m = FileMatcher::rooted("/project", Some(&["lib/*.rb".to_string()]), None).unwrap();
        assert!(m.is_target(Path::new("lib/foo.rb")));
        assert!(!m.is_target(Path::new("lib/nested/foo.rb")), "* must not cross a separator");
    }
}
