//! `Bundler/GemFilename`, ported from RuboCop's
//! `lib/rubocop/cop/bundler/gem_filename.rb`.
//!
//! This cop never inspects the AST at all (upstream's `on_new_investigation`
//! looks only at `processed_source.file_path`), so it subscribes to no node
//! kinds and does everything in [`Rule::file_end`], mirroring
//! `Lint/EmptyFile`'s file-level shape. The offense (when any) is reported
//! at [`linter::GLOBAL_SPAN`] via [`Context::report_global`], matching
//! upstream's `add_global_offense`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};

/// RuboCop's `MSG_GEMFILE_REQUIRED`.
const MSG_GEMFILE_REQUIRED: &str =
    "`gems.rb` file was found but `Gemfile` is required (file path: {}).";
/// RuboCop's `MSG_GEMS_RB_REQUIRED`.
const MSG_GEMS_RB_REQUIRED: &str =
    "`Gemfile` was found but `gems.rb` file is required (file path: {}).";
/// RuboCop's `MSG_GEMFILE_MISMATCHED`.
const MSG_GEMFILE_MISMATCHED: &str =
    "Expected a `Gemfile.lock` with `Gemfile` but found `gems.locked` file (file path: {}).";
/// RuboCop's `MSG_GEMS_RB_MISMATCHED`.
const MSG_GEMS_RB_MISMATCHED: &str =
    "Expected a `gems.locked` file with `gems.rb` but found `Gemfile.lock` (file path: {}).";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Gemfile,
    GemsRb,
}

/// Verifies that a project contains a `Gemfile` or `gems.rb` file and the
/// correct associated lock file, based on the configured style.
#[derive(Debug, Clone)]
pub struct GemFilename {
    style: Style,
}

impl GemFilename {
    /// RuboCop's `expected_gemfile?`.
    fn expected_gemfile(&self, basename: &str) -> bool {
        match self.style {
            Style::Gemfile => matches!(basename, "Gemfile" | "Gemfile.lock"),
            Style::GemsRb => matches!(basename, "gems.rb" | "gems.locked"),
        }
    }
}

impl Rule for GemFilename {
    const META: RuleMeta = RuleMeta {
        name: "Bundler/GemFilename",
        department: Department::Bundler,
        summary: "Verifies that a project contains Gemfile or gems.rb file and correct \
                  associated lock file based on the configuration.",
        explanation: "\
Verifies that a project contains Gemfile or gems.rb file and correct
associated lock file based on the configuration.

```ruby
# EnforcedStyle: Gemfile (default)

# bad
Project contains gems.rb and gems.locked files

# bad
Project contains Gemfile and gems.locked file

# good
Project contains Gemfile and Gemfile.lock
```

```ruby
# EnforcedStyle: gems.rb

# bad
Project contains Gemfile and Gemfile.lock files

# bad
Project contains gems.rb and Gemfile.lock file

# good
Project contains gems.rb and gems.locked files
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("Gemfile"),
            allowed: &["Gemfile", "gems.rb"],
            doc: "Which gem file naming convention to enforce.",
        }],
        blind_spots: "\
Only the currently linted file's own basename is checked, matching
upstream's `on_new_investigation`; this cop never inspects the filesystem
for the *other* file of the pair, so linting `Gemfile.lock` in isolation
(with the `gems.rb` style) still reports a mismatch even if a `gems.rb` was
in fact never created, and vice versa -- exactly upstream's behavior, since
it too checks only the one file it happens to be linting.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "gems.rb" => Style::GemsRb,
            _ => Style::Gemfile,
        };
        Ok(Self { style })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let path = ctx.source().path();
        let Some(basename) = path.file_name().and_then(|name| name.to_str()) else { return };
        if self.expected_gemfile(basename) {
            return;
        }
        let file_path = path.display().to_string();
        let message = match (self.style, basename) {
            (Style::Gemfile, "gems.rb") => Some(MSG_GEMFILE_REQUIRED),
            (Style::Gemfile, "gems.locked") => Some(MSG_GEMFILE_MISMATCHED),
            (Style::GemsRb, "Gemfile") => Some(MSG_GEMS_RB_REQUIRED),
            (Style::GemsRb, "Gemfile.lock") => Some(MSG_GEMS_RB_MISMATCHED),
            _ => None,
        };
        if let Some(template) = message {
            ctx.report_global(&Self::META, template.replacen("{}", &file_path, 1));
        }
    }
}
