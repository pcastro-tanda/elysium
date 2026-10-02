use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use regex::Regex;

use crate::defaults::DEFAULT_CONFIG;
use crate::yaml::YamlValue;

/// True when some `/`-separated component of `path` starts with `.` (other
/// than `.`/`..`), i.e. the path RuboCop's `TargetFinder` considers hidden
/// (`path.include?("/.")`, applied here per-component instead of as a
/// substring test so it agrees with `Path`'s own segmentation).
pub fn is_hidden_path(path: &Path) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(name) => name.to_str().is_some_and(|s| s.starts_with('.')),
        _ => false,
    })
}

/// RuboCop's `AllCops/Include` defaults, read straight out of the embedded
/// `config/default.yml` so the two can never drift.
pub static DEFAULT_INCLUDE: LazyLock<Vec<YamlValue>> = LazyLock::new(|| all_cops_list("Include"));

/// RuboCop's `AllCops/Exclude` defaults, read straight out of the embedded
/// `config/default.yml`.
pub static DEFAULT_EXCLUDE: LazyLock<Vec<YamlValue>> = LazyLock::new(|| all_cops_list("Exclude"));

fn all_cops_list(key: &str) -> Vec<YamlValue> {
    DEFAULT_CONFIG
        .get_mapping("AllCops")
        .map(|all_cops| all_cops.get_pattern_list(key))
        .unwrap_or_default()
}

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
    /// `Cop::Base#file_name_matches_any?` tries a cop's `Exclude` against
    /// the relative path before the absolute one; `Config#file_to_exclude?`
    /// (`AllCops`) only ever tries the absolute path. Globs agree either way
    /// (they were absolutised on load), `!ruby/regexp` patterns don't.
    regexp_excludes_relative: bool,
}

/// One side (`Include` or `Exclude`) of a matcher.
#[derive(Debug, Clone)]
struct Clusivity {
    relative: GlobSet,
    absolute: GlobSet,
    has_absolute: bool,
    /// Subset of `relative`/`absolute` whose source pattern names a literal
    /// dot path component (e.g. `**/.simplecov`), mirroring the only way
    /// RuboCop's `File.fnmatch?` (no `FNM_DOTMATCH`) can match a hidden
    /// path: the pattern must spell the leading dot itself rather than rely
    /// on a wildcard.
    dot_relative: GlobSet,
    dot_absolute: GlobSet,
    has_dot_absolute: bool,
    /// True when the configuration does not set this key at all, in which case
    /// RuboCop's `file_name_matches_any?` returns its default answer.
    unset: bool,
    /// `!ruby/regexp` entries, matched unanchored like Ruby's `Regexp#match?`.
    regexps: Vec<Regex>,
}

/// True when some `/`-separated component of `pattern` is a literal dot
/// path (starts with `.`, and isn't `.`/`..`). Wildcards such as `*.rb` or
/// `**` don't count: RuboCop's fnmatch-based matching only crosses into a
/// hidden path when the pattern names the dot explicitly.
fn has_explicit_dot_segment(pattern: &str) -> bool {
    pattern.split('/').any(|segment| segment.starts_with('.') && segment != "." && segment != "..")
}

/// A `!ruby/regexp` source (`/body/flags`) as a Rust regex; `None` when the
/// body doesn't compile, which then never matches (RuboCop would raise).
fn compile_regexp(source: &str) -> Option<Regex> {
    let (body, flags) = source.strip_prefix('/').and_then(|rest| rest.rsplit_once('/'))?;
    let mut inline = String::new();
    for flag in flags.chars() {
        match flag {
            'i' => inline.push('i'),
            // Ruby's `m` makes `.` match a newline: Rust's `s`.
            'm' => inline.push('s'),
            'x' => inline.push('x'),
            _ => {}
        }
    }
    let pattern = if inline.is_empty() { body.to_string() } else { format!("(?{inline}){body}") };
    Regex::new(&pattern).ok()
}

impl Clusivity {
    fn build(patterns: Option<&[YamlValue]>) -> Result<Self, globset::Error> {
        let Some(patterns) = patterns else {
            return Ok(Self {
                relative: GlobSet::empty(),
                absolute: GlobSet::empty(),
                has_absolute: false,
                dot_relative: GlobSet::empty(),
                dot_absolute: GlobSet::empty(),
                has_dot_absolute: false,
                unset: true,
                regexps: Vec::new(),
            });
        };
        let mut relative = GlobSetBuilder::new();
        let mut absolute = GlobSetBuilder::new();
        let mut has_absolute = false;
        let mut dot_relative = GlobSetBuilder::new();
        let mut dot_absolute = GlobSetBuilder::new();
        let mut has_dot_absolute = false;
        let mut regexps = Vec::new();
        for pattern in patterns {
            let pattern = match pattern {
                YamlValue::String(pattern) => pattern,
                YamlValue::Regexp(source) => {
                    regexps.extend(compile_regexp(source));
                    continue;
                }
                _ => continue,
            };
            let Ok(glob) = compile(pattern) else {
                // RuboCop's `File.fnmatch?` treats a malformed pattern as a
                // literal that simply never matches a real path.
                continue;
            };
            let dotted = has_explicit_dot_segment(pattern);
            // `Config#match_relative_or_absolute_path?` (RuboCop 1.91.0)
            // matches an absolute pattern, or one reaching out of the config
            // directory, against the absolute path only.
            if Path::new(pattern).is_absolute() || pattern.starts_with("..") {
                has_absolute = true;
                absolute.add(glob.clone());
                if dotted {
                    has_dot_absolute = true;
                    dot_absolute.add(glob);
                }
            } else {
                relative.add(glob.clone());
                if dotted {
                    dot_relative.add(glob);
                }
            }
        }
        Ok(Self {
            relative: relative.build()?,
            absolute: absolute.build()?,
            has_absolute,
            dot_relative: dot_relative.build()?,
            dot_absolute: dot_absolute.build()?,
            has_dot_absolute,
            unset: false,
            regexps,
        })
    }

    fn regexp_match(&self, path: &Path) -> bool {
        !self.regexps.is_empty()
            && path.to_str().is_some_and(|path| self.regexps.iter().any(|re| re.is_match(path)))
    }

    /// Exclude-side matching: a relative pattern is tried against the path
    /// relative to the configuration's directory and an absolute one against
    /// the absolute path. RuboCop's `Config#file_to_exclude?` only ever
    /// matches the absolute path, which agrees because
    /// `Config#make_excludes_absolute` has already absolutised every
    /// `Exclude` entry a configuration file declares. A `!ruby/regexp` is
    /// tried against the absolute path, and against the relative one first
    /// when `regexp_relative` (a cop's `Exclude`).
    fn is_match(&self, root: &Path, relative: &Path, default: bool, regexp_relative: bool) -> bool {
        if self.unset {
            return default;
        }
        if self.relative.is_match(relative) || (regexp_relative && self.regexp_match(relative)) {
            return true;
        }
        if self.has_absolute && self.absolute.is_match(root.join(relative)) {
            return true;
        }
        self.regexp_match(&root.join(relative))
    }

    /// `Config#file_to_include?` as of RuboCop 1.91.0, which routes each
    /// pattern to exactly one of the two paths
    /// (`Config#match_relative_or_absolute_path?`): the absolute one when the
    /// pattern is itself absolute or reaches out of the configuration's
    /// directory, or when the file lies outside that directory; the relative
    /// one otherwise. Before 1.91.0 every pattern was tried against both.
    /// A `!ruby/regexp` is never absolute (`Regexp#to_s` is `(?-mix:...)`),
    /// so it follows the file: relative unless the file is outside.
    fn is_include_match(&self, root: &Path, relative: &Path, default: bool) -> bool {
        if self.unset {
            return default;
        }
        if relative.starts_with("..") {
            let absolute = crate::paths::normalize(&root.join(relative));
            return self.relative.is_match(&absolute)
                || (self.has_absolute && self.absolute.is_match(&absolute))
                || self.regexp_match(&absolute);
        }
        if self.relative.is_match(relative) || self.regexp_match(relative) {
            return true;
        }
        self.has_absolute && self.absolute.is_match(root.join(relative))
    }

    /// Same as `is_match`, restricted to patterns that name a dot path
    /// component explicitly. Used for paths RuboCop considers hidden, which
    /// only ever match such a pattern (see `has_explicit_dot_segment`).
    fn is_hidden_match(&self, root: &Path, relative: &Path) -> bool {
        if self.unset {
            return false;
        }
        if self.dot_relative.is_match(relative) {
            return true;
        }
        self.has_dot_absolute && self.dot_absolute.is_match(root.join(relative))
    }
}

impl FileMatcher {
    /// Builds a matcher from include and exclude pattern lists, rooted at the
    /// current directory.
    pub fn new(include: &[&str], exclude: &[&str]) -> Result<Self, globset::Error> {
        let owned =
            |p: &[&str]| p.iter().map(|s| YamlValue::String((*s).to_string())).collect::<Vec<_>>();
        Self::rooted(PathBuf::new(), Some(&owned(include)), Some(&owned(exclude)))
    }

    /// Builds the `AllCops` matcher for `root`, where `None` means the
    /// configuration does not set that key: an unset `Include` includes
    /// everything and an unset `Exclude` excludes nothing. Patterns are
    /// strings (globs) or `!ruby/regexp` values; anything else is ignored.
    pub fn rooted(
        root: impl Into<PathBuf>,
        include: Option<&[YamlValue]>,
        exclude: Option<&[YamlValue]>,
    ) -> Result<Self, globset::Error> {
        Ok(Self {
            root: root.into(),
            include: Clusivity::build(include)?,
            exclude: Clusivity::build(exclude)?,
            regexp_excludes_relative: false,
        })
    }

    /// Like [`FileMatcher::rooted`], for one cop's `Include`/`Exclude`
    /// (`Cop::Base#file_name_matches_any?`).
    pub fn rooted_cop(
        root: impl Into<PathBuf>,
        include: Option<&[YamlValue]>,
        exclude: Option<&[YamlValue]>,
    ) -> Result<Self, globset::Error> {
        Ok(Self { regexp_excludes_relative: true, ..Self::rooted(root, include, exclude)? })
    }

    /// RuboCop's defaults.
    pub fn rubocop_defaults() -> Self {
        Self::rooted(PathBuf::new(), Some(&DEFAULT_INCLUDE), Some(&DEFAULT_EXCLUDE))
            .expect("default patterns are valid")
    }

    /// The directory relative paths are resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// True when `relative` (a path relative to the project root) matches an
    /// include pattern.
    pub fn is_included(&self, relative: &Path) -> bool {
        self.include.is_include_match(&self.root, relative, true)
    }

    /// True when `relative` is hidden (see [`is_hidden_path`]) and matches an
    /// include pattern that itself names the dot path component explicitly,
    /// mirroring RuboCop's `TargetFinder#to_inspect?` for a hidden file.
    pub fn is_included_hidden(&self, relative: &Path) -> bool {
        self.include.is_hidden_match(&self.root, relative)
    }

    /// True when `relative` matches an exclude pattern.
    pub fn is_excluded(&self, relative: &Path) -> bool {
        self.exclude.is_match(&self.root, relative, false, self.regexp_excludes_relative)
    }

    /// Included and not excluded.
    pub fn is_target(&self, relative: &Path) -> bool {
        self.is_included(relative) && !self.is_excluded(relative)
    }
}

/// `**` is only special as a whole path component (`**/`); `File.fnmatch?`
/// treats it anywhere else (`danger/**/**.rb`) as a plain `*`, which globset
/// would reject.
fn compile(pattern: &str) -> Result<Glob, globset::Error> {
    let normalized = pattern
        .split('/')
        .map(|segment| {
            if segment != "**" && segment.contains("**") {
                let mut collapsed = segment.to_string();
                while collapsed.contains("**") {
                    collapsed = collapsed.replace("**", "*");
                }
                std::borrow::Cow::Owned(collapsed)
            } else {
                std::borrow::Cow::Borrowed(segment)
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    GlobBuilder::new(&normalized).literal_separator(true).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn globs(patterns: &[&str]) -> Vec<YamlValue> {
        patterns.iter().map(|p| YamlValue::String((*p).to_string())).collect()
    }

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
            Some(&globs(&["**/*.rb"])),
            Some(&globs(&["/project/db/schema.rb", "spec/**/*"])),
        )
        .unwrap();
        assert!(m.is_target(Path::new("app/user.rb")));
        assert!(!m.is_target(Path::new("db/schema.rb")));
        assert!(!m.is_target(Path::new("spec/models/user_spec.rb")));
    }

    #[test]
    fn unset_clusivity_uses_rubocop_defaults() {
        let m = FileMatcher::rooted("/project", None, Some(&globs(&["lib/**/*"]))).unwrap();
        // An unset `Include` includes everything; an unset `Exclude` excludes nothing.
        assert!(m.is_target(Path::new("anything.txt")));
        assert!(!m.is_target(Path::new("lib/foo.rb")));
        let m = FileMatcher::rooted("/project", Some(&globs(&["lib/*.rb"])), None).unwrap();
        assert!(m.is_target(Path::new("lib/foo.rb")));
        assert!(!m.is_target(Path::new("lib/nested/foo.rb")), "* must not cross a separator");
    }

    #[test]
    fn hidden_paths_need_an_explicit_dot_pattern() {
        let m = FileMatcher::rubocop_defaults();
        assert!(is_hidden_path(Path::new(".simplecov")));
        assert!(is_hidden_path(Path::new(".devcontainer/x.rb")));
        assert!(!is_hidden_path(Path::new("lib/a.rb")));

        // `**/*.rb` never reaches into a hidden path.
        assert!(!m.is_included_hidden(Path::new(".devcontainer/x.rb")));
        // `**/.simplecov` spells out the dot, so it does.
        assert!(m.is_included_hidden(Path::new(".simplecov")));
        assert!(m.is_included_hidden(Path::new("lib/.simplecov")));
    }

    #[test]
    fn defaults_track_the_embedded_default_yml() {
        // RuboCop 1.91.0 dropped `**/*.fcgi`, `**/*.god`, `**/*.rbuild`,
        // `**/*.rbx`, `**/*.watchr`, `**/Cheffile` and `**/Vagabondfile` from
        // `AllCops/Include`.
        let m = FileMatcher::rubocop_defaults();
        assert!(!m.is_target(Path::new("dispatch.fcgi")));
        assert!(!m.is_target(Path::new("Cheffile")));
        assert!(!m.is_target(Path::new("Vagabondfile")));
        assert!(m.is_target(Path::new("Vagrantfile")));
        assert!(m.is_target(Path::new("Steepfile")));
    }

    #[test]
    fn includes_match_a_file_outside_the_config_directory_by_absolute_path() {
        // `Config#match_relative_or_absolute_path?` (RuboCop 1.91.0): a file
        // whose relative path escapes the configuration's directory is only
        // ever matched as an absolute path, and so is a pattern that itself
        // reaches out of it.
        let exclude: [YamlValue; 0] = [];
        let m = FileMatcher::rooted(
            "/project/sub",
            Some(&globs(&["**/*.rb"])),
            Some(exclude.as_slice()),
        )
        .unwrap();
        assert!(m.is_included(Path::new("app/user.rb")));
        // `**/*.rb` is tried against `/project/other/user.rb`, which it matches.
        assert!(m.is_included(Path::new("../other/user.rb")));
        assert!(!m.is_included(Path::new("../other/user.txt")));

        // A relative pattern spelled with `..` is only ever tried against the
        // expanded absolute path, which it can never match -- exactly as
        // `File.fnmatch?('../shared/*.rb', '/project/shared/user.rb')` cannot.
        let m = FileMatcher::rooted(
            "/project/sub",
            Some(&globs(&["../shared/*.rb"])),
            Some(exclude.as_slice()),
        )
        .unwrap();
        assert!(!m.is_included(Path::new("../shared/user.rb")));
        assert!(!m.is_included(Path::new("app/user.rb")));
    }

    #[test]
    fn regexp_patterns_follow_rubocop_path_routing() {
        let regexp = |source: &str| YamlValue::Regexp(source.to_string());
        let exclude = [regexp(r"/db\/migrate\/201[0-6].*$/"), regexp(r"/\Aspec\//")];
        let include = Some(globs(&["**/*.rb"]));

        // `Config#file_to_exclude?` matches the absolute path only, so an
        // `\A`-anchored regexp written against relative paths never hits.
        let all_cops = FileMatcher::rooted("/p", include.as_deref(), Some(&exclude)).unwrap();
        assert!(!all_cops.is_target(Path::new("db/migrate/2015_a.rb")));
        assert!(all_cops.is_target(Path::new("db/migrate/2017_a.rb")));
        assert!(all_cops.is_target(Path::new("spec/a_spec.rb")));

        // A cop's `Exclude` tries the relative path first.
        let cop = FileMatcher::rooted_cop("/p", include.as_deref(), Some(&exclude)).unwrap();
        assert!(!cop.is_target(Path::new("spec/a_spec.rb")));
        assert!(!cop.is_target(Path::new("db/migrate/2016_a.rb")));

        // `Include` regexps see the relative path; `i` is case-insensitive.
        let m = FileMatcher::rooted("/p", Some(&[regexp(r"/\Aapp\/.*\.RB\z/i")]), None).unwrap();
        assert!(m.is_target(Path::new("app/models/user.rb")));
        assert!(!m.is_target(Path::new("lib/app/user.rb")));
    }
}
