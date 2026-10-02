//! `Style/ExactRegexpMatch`, ported from RuboCop's
//! `lib/rubocop/cop/style/exact_regexp_match.rb`.
//!
//! Upstream parses the regexp body with the `regexp_parser` gem and checks
//! its token stream is exactly `[:bos, :literal, :eos]` (`\A`, a plain
//! literal run, `\z`) with no quantifier on the literal token. This port
//! re-implements just that one shape directly over the raw regexp source
//! bytes in [`exact_literal`]: strip a leading `\A`/trailing `\z`, then scan
//! the remainder rejecting any unescaped regex metacharacter (which would
//! produce a `:meta`/`:quantifier` token upstream, failing the 3-token
//! check) and any backslash escape that is not simply a punctuation
//! character escaped down to its literal self (which would produce some
//! other `:escape` token type upstream instead of extending the `:literal`
//! run). The `(regopt)` node-pattern slot (zero children) becomes
//! [`has_no_flags`]: every option/encoding-letter flag must be unset.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::RegularExpressionNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG_PREFIX: &str = "Use `";

/// Checks for exact regexp match inside `Regexp` literals.
#[derive(Debug, Clone)]
pub struct ExactRegexpMatch;

impl Rule for ExactRegexpMatch {
    const META: RuleMeta = RuleMeta {
        name: "Style/ExactRegexpMatch",
        department: Department::Style,
        summary: "Checks for exact regexp match inside Regexp literals.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "The upstream `regexp_parser`-based tokenizer is reimplemented as a direct \
                      scan for the single `\\A<literal>\\z` shape (see the module doc); regexes \
                      regexp_parser itself fails to parse (e.g. an incomplete `\\P` property \
                      escape) are treated the same as any other non-matching shape (no offense) \
                      rather than needing a distinct parse-failure path.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !matches!(call.name().as_slice(), b"=~" | b"===" | b"!~" | b"match" | b"match?") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(args) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        let [arg] = items.as_slice() else { return };
        let Some(regexp) = arg.as_regular_expression_node() else { return };
        if !has_no_flags(&regexp) {
            return;
        }
        let Some(literal) = exact_literal(regexp.content_loc().as_slice()) else { return };

        let receiver_src = String::from_utf8_lossy(ctx.text(receiver.span())).into_owned();
        let method = if call.name().as_slice() == b"!~" { "!=" } else { "==" };
        let string = escape_single_quotes(&literal);
        let prefer = format!("{receiver_src} {method} '{string}'");
        let message = format!("{MSG_PREFIX}{prefer}`.");

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(node.span(), prefer.into_bytes())],
            },
        );
    }
}

/// Every regexp-option/encoding-letter flag is unset: upstream's
/// `(regopt)` node-pattern slot requiring zero children.
fn has_no_flags(regexp: &RegularExpressionNode<'_>) -> bool {
    !(regexp.is_ignore_case()
        || regexp.is_extended()
        || regexp.is_multi_line()
        || regexp.is_once()
        || regexp.is_euc_jp()
        || regexp.is_ascii_8bit()
        || regexp.is_windows_31j()
        || regexp.is_utf_8())
}

/// `\A<literal>\z` with no other regex metacharacters, quantifiers, or
/// non-punctuation escapes in the literal run. Returns the literal's
/// unescaped text (an escaped punctuation character like `\.` or `\'`
/// unescapes to its bare character, matching `regexp_parser`'s `.text`).
fn exact_literal(content: &[u8]) -> Option<String> {
    let middle = content.strip_prefix(b"\\A")?.strip_suffix(b"\\z")?;
    let mut text = Vec::with_capacity(middle.len());
    let mut i = 0;
    while i < middle.len() {
        let b = middle[i];
        if b == b'\\' {
            let next = *middle.get(i + 1)?;
            if next.is_ascii_punctuation() {
                text.push(next);
                i += 2;
                continue;
            }
            return None;
        }
        if matches!(
            b,
            b'.' | b'*'
                | b'+'
                | b'?'
                | b'('
                | b')'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'|'
                | b'^'
                | b'$'
        ) {
            return None;
        }
        text.push(b);
        i += 1;
    }
    String::from_utf8(text).ok()
}

/// RuboCop's `escape_single_quotes`.
fn escape_single_quotes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\'' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
