//! `Style/PercentQLiterals`, ported from RuboCop's
//! `lib/rubocop/cop/style/percent_q_literals.rb`, plus the `PercentLiteral`
//! mixin it includes.
//!
//! `on_str` only fires for a plain (non-interpolated) `StringNode`; a
//! `%Q(...)` that actually interpolates is Prism's
//! `InterpolatedStringNode`, which this cop never visits -- matching
//! upstream's "accepts %Q with interpolation" behavior for free.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, Parsed};
use ruby_source::SourceFile;

/// RuboCop's `LOWER_CASE_Q_MSG`.
const LOWER_CASE_Q_MSG: &str = "Do not use `%Q` unless interpolation is needed. Use `%q`.";
/// RuboCop's `UPPER_CASE_Q_MSG`.
const UPPER_CASE_Q_MSG: &str = "Use `%Q` instead of `%q`.";

/// RuboCop's `ConfigurableEnforcedStyle` `style` for this cop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Use `%q` when possible, `%Q` when necessary.
    LowerCaseQ,
    /// Always use `%Q`.
    UpperCaseQ,
}

/// Checks if uses of %Q/%q match the configured preference.
#[derive(Debug, Clone)]
pub struct PercentQLiterals {
    style: Style,
}

impl Rule for PercentQLiterals {
    const META: RuleMeta = RuleMeta {
        name: "Style/PercentQLiterals",
        department: Department::Style,
        summary: "Checks if uses of %Q/%q match the configured preference.",
        explanation: "Checks for usage of the %Q() syntax when %q() would do.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode],
        config: &[ConfigOption {
            name: "EnforcedStyle",
            default: ConfigDefault::Str("lower_case_q"),
            allowed: &["lower_case_q", "upper_case_q"],
            doc: "The preferred style for `%q`/`%Q` literals.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "upper_case_q" => Style::UpperCaseQ,
            _ => Style::LowerCaseQ,
        };
        Ok(Self { style })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(str_node) = node.as_string_node() else { return };
        let Some(opening) = str_node.opening_loc() else { return };
        let opening_bytes = ctx.text(opening.span());
        // RuboCop's `type(node)`: the opening token minus its last byte (the
        // delimiter character), e.g. `%Q(` -> `%Q`.
        if opening_bytes.len() < 2 {
            return;
        }
        let literal_type = &opening_bytes[..opening_bytes.len() - 1];
        let is_upper = literal_type == b"%Q";
        let is_lower = literal_type == b"%q";
        if !is_upper && !is_lower {
            return;
        }
        // Whitequark parses a multi-line `%Q` (or `"..."`) with no actual
        // interpolation as a `dstr` of per-line `str` parts, so upstream's
        // `on_str` never fires on it (those child `str`s have no
        // `loc.begin`). Prism keeps it a single `StringNode`, so mirror
        // upstream's blind spot by skipping multi-line `%Q` here; `%q`
        // stays a plain `str` either way and is still checked.
        if is_upper && !ctx.is_single_line(node.span()) {
            return;
        }

        // RuboCop's `correct_literal_style?`.
        let correct = match self.style {
            Style::LowerCaseQ => is_lower,
            Style::UpperCaseQ => is_upper,
        };
        if correct {
            return;
        }

        let span = node.span();
        let raw = ctx.text(span).to_vec();
        // RuboCop's `corrected`: swap the case of the letter right after
        // `%` (`src.sub(src[1], src[1].swapcase)` always matches there
        // first, since index 0 is `%`).
        let mut corrected = raw.clone();
        corrected[1] = if is_upper { b'q' } else { b'Q' };

        // RuboCop's semantics guard: only offend if re-parsing the
        // corrected source yields the same string value, i.e. the case
        // flip doesn't turn the literal dynamic or change what it escapes.
        if !reparse_matches(&corrected, str_node.unescaped()) {
            return;
        }

        let msg = match self.style {
            Style::LowerCaseQ => LOWER_CASE_Q_MSG,
            Style::UpperCaseQ => UPPER_CASE_Q_MSG,
        };
        let fix =
            Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, corrected)] };
        ctx.report_with_fix(&Self::META, opening.span(), msg, fix);
    }
}

/// Re-parses `corrected` standalone and checks it is still a lone plain
/// string literal whose unescaped content matches `original_unescaped`.
/// RuboCop's `ast = parse(corrected(node.source)).ast; node.children !=
/// ast&.children`.
fn reparse_matches(corrected: &[u8], original_unescaped: &[u8]) -> bool {
    let source = SourceFile::new("(percent_q_literals)", corrected.to_vec());
    let parsed = Parsed::parse(&source);
    if parsed.has_errors() {
        return false;
    }
    let root = parsed.root();
    let Some(program) = root.as_program_node() else { return false };
    let body = program.statements().body();
    let [only] = body.iter().collect::<Vec<_>>()[..] else { return false };
    let Some(reparsed_str) = only.as_string_node() else { return false };
    reparsed_str.unescaped() == original_unescaped
}
