//! `Lint/InterpolationCheck`, ported from RuboCop's
//! `lib/rubocop/cop/lint/interpolation_check.rb`.
//!
//! `node.parent&.regexp_type?` never applies here: whitequark unifies
//! string/regexp literals under `str`/`dstr`, so a `#{...}`-shaped fragment
//! inside a regexp without real interpolation shows up as a `str` child of a
//! `regexp` node. Prism keeps regexp literals as their own node kinds
//! (`RegularExpressionNode`/`InterpolatedRegularExpressionNode`), which never
//! contain a `StringNode`, so that guard has no Prism equivalent to port.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext::is_heredoc, LocationExt as _, Node, NodeExt as _, NodeKind, Parsed};
use ruby_source::SourceFile;

const MSG: &str = "Interpolation in single quoted string detected. Use double quoted strings if \
                    you need interpolation.";

/// Checks for interpolation in a single quoted string.
#[derive(Debug, Clone)]
pub struct InterpolationCheck;

impl Rule for InterpolationCheck {
    const META: RuleMeta = RuleMeta {
        name: "Lint/InterpolationCheck",
        department: Department::Lint,
        summary: "Checks for interpolation in a single quoted string.",
        explanation: "Checks for interpolation in a single quoted string. A single-quoted \
                      string that happens to contain `#{...}` never actually interpolates; \
                      this usually means the author meant to use a double-quoted string.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode],
        config: &[],
        blind_spots: "`node.parent&.regexp_type?` is unreachable under Prism: regexp literals \
                       are never composed of `StringNode`s, so it is not ported.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(str_node) = node.as_string_node() else { return };
        let span = node.span();
        let raw = ctx.text(span).to_vec();

        if !has_unescaped_interpolation(&raw) {
            return;
        }
        if is_heredoc(node) {
            return;
        }
        let (Some(opening), Some(closing)) = (str_node.opening_loc(), str_node.closing_loc())
        else {
            return;
        };
        if !valid_as_double_quoted(&raw) {
            return;
        }

        let (start_token, end_token): (&[u8], &[u8]) =
            if raw.contains(&b'"') { (b"%{", b"}") } else { (b"\"", b"\"") };
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![
                Edit::replace(opening.span(), start_token.to_vec()),
                Edit::replace(closing.span(), end_token.to_vec()),
            ],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}

/// `/(?<!\\)#\{.*\}/.match?(node.source)`, without lookbehind support: scans
/// for an unescaped `#{` followed by a `}` on the same line.
fn has_unescaped_interpolation(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'#' && bytes[i + 1] == b'{' && !(i > 0 && bytes[i - 1] == b'\\') {
            let mut j = i + 2;
            while j < bytes.len() && bytes[j] != b'\n' {
                if bytes[j] == b'}' {
                    return true;
                }
                j += 1;
            }
        }
        i += 1;
    }
    false
}

/// `valid_syntax?` (RuboCop 1.91): the source requoted the way the fix
/// would requote it (`%{...}` when it contains `"`, else `"..."`) must parse
/// cleanly to a lone interpolated string (`ast.dstr_type?`).
fn valid_as_double_quoted(raw: &[u8]) -> bool {
    let (open, close): (&[u8], &[u8]) =
        if raw.contains(&b'"') { (b"%{", b"}") } else { (b"\"", b"\"") };
    let (head, body) = match raw.strip_prefix(b"'") {
        Some(rest) => (open, rest),
        None => (&b""[..], raw),
    };
    let (body, tail) = match body.strip_suffix(b"'") {
        Some(rest) => (rest, close),
        None => (body, &b""[..]),
    };
    let candidate = [head, body, tail].concat();
    let source = SourceFile::new("(interpolation_check)", candidate);
    let parsed = Parsed::parse(&source);
    if parsed.has_errors() {
        return false;
    }
    let root = parsed.root();
    let Some(program) = root.as_program_node() else { return false };
    let body = program.statements().body();
    body.len() == 1 && body.iter().all(|n| n.as_interpolated_string_node().is_some())
}
