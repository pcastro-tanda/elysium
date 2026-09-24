//! Applying the byte edits rules attach to their diagnostics.
//!
//! Fixes are applied in diagnostic order (which the engine has already
//! sorted by position); a fix that would touch bytes an earlier fix already
//! claimed is skipped and left for the next iteration, exactly like
//! RuboCop's clobbering-correction handling.

use ruby_ast::{ParseOptions, Parsed};
use ruby_source::{SourceFile, Span};

use crate::diagnostic::{Applicability, Diagnostic, Edit};
use crate::engine::lint_parsed_with_injected;
use crate::rule::Dispatch;
use crate::settings::FileSettings;

/// Iteration ceiling for [`fix_file`]. RuboCop uses 200; a rule needing
/// more than a handful of rounds is looping, not converging.
pub const MAX_FIX_ITERATIONS: u32 = 20;

/// What one fix pass (or one [`fix_file`] run) did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FixReport {
    /// Fixes whose edits were applied.
    pub applied: usize,
    /// Fixes skipped because they overlapped an already-applied fix.
    pub skipped_overlapping: usize,
    /// Fixes skipped because they are unsafe and unsafe fixes were not allowed.
    pub skipped_unsafe: usize,
    /// Lint/apply rounds performed.
    pub iterations: u32,
    /// A round produced source Prism could not parse; its output was discarded.
    pub introduced_syntax_error: bool,
}

/// Result of [`fix_file`].
#[derive(Debug)]
pub struct FixOutcome {
    /// Final source bytes. Equal to the input when nothing applied.
    pub bytes: Vec<u8>,
    /// Diagnostics of the last successful lint of [`FixOutcome::bytes`].
    pub diagnostics: Vec<Diagnostic>,
    /// What was applied, and why anything was not.
    pub report: FixReport,
}

impl FixOutcome {
    /// True when fixing changed the source.
    pub fn changed(&self, original: &[u8]) -> bool {
        self.bytes != original
    }
}

/// True when two edit spans cannot both be applied. Insertions (empty
/// spans) only conflict with a replacement they sit strictly inside, or
/// with another insertion at the same offset.
fn conflicts(a: Span, b: Span) -> bool {
    match (a.is_empty(), b.is_empty()) {
        (true, true) => a.start == b.start,
        (true, false) => b.start < a.start && a.start < b.end,
        (false, true) => a.start < b.start && b.start < a.end,
        (false, false) => a.start < b.end && b.start < a.end,
    }
}

/// Applies non-overlapping fixes of allowed applicability to `bytes` in one
/// pass. Fixes are taken in diagnostic order; a fix whose edits overlap an
/// already-accepted edit is skipped.
pub fn apply_fixes(
    bytes: &[u8],
    diagnostics: &[Diagnostic],
    allow_unsafe: bool,
) -> (Vec<u8>, FixReport) {
    let mut report = FixReport { iterations: 1, ..FixReport::default() };
    let mut accepted: Vec<&Edit> = Vec::new();
    let mut claimed: Vec<Span> = Vec::new();

    for diagnostic in diagnostics {
        let Some(fix) = &diagnostic.fix else { continue };
        if fix.edits.is_empty() {
            continue;
        }
        if matches!(fix.applicability, Applicability::Unsafe) && !allow_unsafe {
            report.skipped_unsafe += 1;
            continue;
        }
        if fix.edits.iter().any(|edit| claimed.iter().any(|claim| conflicts(*claim, edit.span))) {
            report.skipped_overlapping += 1;
            continue;
        }
        for edit in &fix.edits {
            claimed.push(edit.span);
            accepted.push(edit);
        }
        report.applied += 1;
    }

    if accepted.is_empty() {
        return (bytes.to_vec(), report);
    }

    accepted.sort_by_key(|edit| (edit.span.start, edit.span.end));
    let mut out = Vec::with_capacity(bytes.len());
    let mut pos = 0usize;
    for edit in accepted {
        let range = edit.span.range();
        if range.start < pos || range.end > bytes.len() {
            continue;
        }
        out.extend_from_slice(&bytes[pos..range.start]);
        out.extend_from_slice(&edit.replacement);
        pos = range.end;
    }
    out.extend_from_slice(&bytes[pos..]);
    (out, report)
}

/// Lint, apply, reparse until no fix applies, the source stops changing, or
/// [`MAX_FIX_ITERATIONS`] rounds have run.
///
/// A round whose output Prism cannot parse is rolled back: its bytes are
/// discarded, the loop stops, and [`FixReport::introduced_syntax_error`] is
/// set. `rules` is cloned for every round so per-file rule state starts
/// fresh, as it would for a normal single-pass lint.
pub fn fix_file<D: Dispatch + Clone>(
    source: &SourceFile,
    options: ParseOptions,
    rules: &mut D,
    settings: &FileSettings,
    allow_unsafe: bool,
) -> FixOutcome {
    fix_file_with_injected(source, options, rules, settings, allow_unsafe, &[])
}

/// Like [`fix_file`], but every round's lint pass additionally injects `injected` via
/// [`crate::lint_parsed_with_injected`]. See that function's docs for why fixture replay of
/// `Lint/RedundantCopDisableDirective`'s upstream spec (the only caller) needs this: each round
/// re-resolves `injected`'s `(rule, line)` pairs against that round's own re-parsed source, so a
/// fix that only rewrites text on the injected offense's own line (never deleting whole lines
/// before it) keeps pointing at the right line across iterations.
pub fn fix_file_with_injected<D: Dispatch + Clone>(
    source: &SourceFile,
    options: ParseOptions,
    rules: &mut D,
    settings: &FileSettings,
    allow_unsafe: bool,
    injected: &[(&'static str, u32)],
) -> FixOutcome {
    let path = source.path().to_path_buf();
    let mut current = source.bytes().to_vec();
    let mut report = FixReport::default();
    let mut diagnostics = Vec::new();

    for round in 0..MAX_FIX_ITERATIONS {
        let file = SourceFile::new(path.clone(), current.clone());
        let parsed = Parsed::parse_with(&file, options);
        if parsed.has_errors() && round > 0 {
            // A previous round broke the file: discard it and report the
            // last known-good state.
            report.introduced_syntax_error = true;
            return FixOutcome { bytes: source.bytes().to_vec(), diagnostics, report };
        }

        let mut round_rules = rules.clone();
        let result = lint_parsed_with_injected(&parsed, &mut round_rules, settings, injected);
        report.iterations = round + 1;
        diagnostics = result.diagnostics;

        let (next, pass) = apply_fixes(&current, &diagnostics, allow_unsafe);
        report.applied += pass.applied;
        report.skipped_overlapping += pass.skipped_overlapping;
        report.skipped_unsafe += pass.skipped_unsafe;
        if pass.applied == 0 || next == current {
            return FixOutcome { bytes: current, diagnostics, report };
        }
        current = next;
    }

    // Ran out of rounds: re-lint the final bytes so the returned
    // diagnostics describe what the caller is left with.
    let file = SourceFile::new(path, current.clone());
    let parsed = Parsed::parse_with(&file, options);
    if parsed.has_errors() {
        report.introduced_syntax_error = true;
        return FixOutcome { bytes: source.bytes().to_vec(), diagnostics, report };
    }
    let mut round_rules = rules.clone();
    let result = lint_parsed_with_injected(&parsed, &mut round_rules, settings, injected);
    FixOutcome { bytes: current, diagnostics: result.diagnostics, report }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Fix, Severity};

    fn diagnostic(span: Span, replacement: &str, applicability: Applicability) -> Diagnostic {
        Diagnostic::new("Test/Rule", span, Severity::Convention, "test").with_fix(Fix {
            applicability,
            edits: vec![Edit::replace(span, replacement.as_bytes().to_vec())],
        })
    }

    #[test]
    fn applies_non_overlapping_edits_in_one_pass() {
        let diagnostics = vec![
            diagnostic(Span::new(0, 1), "A", Applicability::Safe),
            diagnostic(Span::new(2, 3), "C", Applicability::Safe),
        ];
        let (out, report) = apply_fixes(b"abc", &diagnostics, false);
        assert_eq!(out, b"AbC");
        assert_eq!(report.applied, 2);
    }

    #[test]
    fn skips_overlapping_and_unsafe_fixes() {
        let diagnostics = vec![
            diagnostic(Span::new(0, 2), "X", Applicability::Safe),
            diagnostic(Span::new(1, 3), "Y", Applicability::Safe),
            diagnostic(Span::new(4, 5), "Z", Applicability::Unsafe),
        ];
        let (out, report) = apply_fixes(b"abcde", &diagnostics, false);
        assert_eq!(out, b"Xcde");
        assert_eq!(report.applied, 1);
        assert_eq!(report.skipped_overlapping, 1);
        assert_eq!(report.skipped_unsafe, 1);

        let (out, report) = apply_fixes(b"abcde", &diagnostics, true);
        assert_eq!(out, b"XcdZ");
        assert_eq!(report.applied, 2);
        assert_eq!(report.skipped_unsafe, 0);
    }
}
