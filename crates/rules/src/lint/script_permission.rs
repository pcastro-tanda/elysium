//! `Lint/ScriptPermission`, ported from RuboCop's
//! `lib/rubocop/cop/lint/script_permission.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};

const SHEBANG: &[u8] = b"#!";

/// Checks if a file which has a shebang line as its first line is granted
/// execute permission.
#[derive(Debug, Clone)]
pub struct ScriptPermission;

impl Rule for ScriptPermission {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ScriptPermission",
        department: Department::Lint,
        summary: "Grant script file execute permission.",
        explanation: "Checks if a file which has a shebang line as \
            its first line is granted execute permission.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "A file's real executable permission bit can only be \
            checked when the linted path is an actual file on disk; sources \
            read from stdin or given a synthetic path are skipped, matching \
            upstream's `@options.key?(:stdin)` guard.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if cfg!(windows) {
            return;
        }
        if !ctx.source().bytes().starts_with(SHEBANG) {
            return;
        }
        let path = ctx.source().path();
        if is_executable(path) {
            return;
        }
        let Some(comment) = ctx.comments().first() else { return };
        let basename = path
            .file_name()
            .map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned());
        let msg = format!("Script file {basename} doesn't have execute permission.");
        ctx.report(&Self::META, comment.span, msg);
    }
}

/// RuboCop's `executable?`: true if the file's stat is executable, or if
/// the underlying path is not a real file (nothing to check, so the cop
/// stays silent rather than guessing).
#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.permissions().mode() & 0o111 != 0,
        Err(_) => true,
    }
}

#[cfg(not(unix))]
fn is_executable(_path: &std::path::Path) -> bool {
    true
}
