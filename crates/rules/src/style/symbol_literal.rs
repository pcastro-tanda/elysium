//! `Style/SymbolLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/style/symbol_literal.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Do not use strings for word-like symbol literals.";

/// Use plain symbols instead of string symbols when possible.
#[derive(Debug, Clone)]
pub struct SymbolLiteral;

impl Rule for SymbolLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/SymbolLiteral",
        department: Department::Style,
        summary: "Use plain symbols instead of string symbols when possible.",
        explanation: "Checks symbol literal syntax.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::SymbolNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let span = node.span();
        let text = ctx.text(span);
        if !is_quoted_word_like_symbol(text) {
            return;
        }

        let replacement: Vec<u8> =
            text.iter().copied().filter(|&b| b != b'"' && b != b'\'').collect();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// RuboCop's `/\A:["'][A-Za-z_]\w*["']\z/.match?(node.source)`: a
/// `:`-prefixed, quoted (single or double) symbol whose inner text is a
/// bare word-like identifier (starts with a letter/underscore, then only
/// word characters) -- i.e. one that could just as well have been written
/// without the quotes. Note the upstream regex uses two independent
/// `["']` classes rather than a backreference, so mismatched quotes (never
/// producible by the parser) would also match; that quirk is preserved
/// here for literalness, though it's unreachable in practice.
fn is_quoted_word_like_symbol(text: &[u8]) -> bool {
    if text.len() < 4 || text[0] != b':' {
        return false;
    }
    let opening = text[1];
    let closing = text[text.len() - 1];
    if !matches!(opening, b'"' | b'\'') || !matches!(closing, b'"' | b'\'') {
        return false;
    }
    let inner = &text[2..text.len() - 1];
    let Some((&first, rest)) = inner.split_first() else { return false };
    (first.is_ascii_alphabetic() || first == b'_')
        && rest.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_')
}
