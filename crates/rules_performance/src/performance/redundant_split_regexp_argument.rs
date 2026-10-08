//! `Performance/RedundantSplitRegexpArgument`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/redundant_split_regexp_argument.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Use string as argument instead of regexp.";

/// `STR_SPECIAL_CHARS`: escape sequences kept as-is in the replacement.
const STR_SPECIAL_CHARS: [&str; 8] = ["\\n", "\\\"", "\\'", "\\\\", "\\t", "\\b", "\\f", "\\r"];

/// Identifies places where `split` argument can be replaced from a
/// deterministic regexp to a string.
#[derive(Debug, Clone)]
pub struct RedundantSplitRegexpArgument;

/// `Util::LITERAL_REGEX`'s single-character alternative.
fn is_literal_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '_' | ' '
                | '\t'
                | '\n'
                | '\x0b'
                | '\x0c'
                | '\r'
                | '-'
                | ','
                | '"'
                | '\''
                | '!'
                | '#'
                | '%'
                | '&'
                | '<'
                | '>'
                | '='
                | ';'
                | ':'
                | '`'
                | '~'
                | '/'
        )
}

/// `/\A(?:#{LITERAL_REGEX})+\Z/.match?(source)`.
fn is_deterministic(source: &str) -> bool {
    let mut chars = source.chars();
    let mut count = 0usize;
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) if !"AbBdDgGhHkpPRwWXsSzZ0123456789".contains(next) => {}
                _ => return false,
            }
        } else if !is_literal_char(c) {
            return false;
        }
        count += 1;
    }
    count > 0
}

fn replacement(content: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();
    let mut pending = false;
    for c in content.chars() {
        if !pending && c == '\\' {
            pending = true;
        } else {
            let mut token = String::new();
            if pending {
                token.push('\\');
                pending = false;
            }
            token.push(c);
            tokens.push(token);
        }
    }
    tokens
        .into_iter()
        .map(|token| {
            if STR_SPECIAL_CHARS.contains(&token.as_str()) {
                token
            } else {
                token.replace('\\', "")
            }
        })
        .collect()
}

impl Rule for RedundantSplitRegexpArgument {
    const META: RuleMeta = RuleMeta {
        name: "Performance/RedundantSplitRegexpArgument",
        department: Department::Performance,
        summary: "Identifies places where `split` argument can be replaced from a deterministic regexp to a string.",
        explanation: "Identifies places where `split` argument can be replaced from a \
                      deterministic regexp to a string.\n\n```ruby\n# bad\n'a,b,c'.split(/,/)\n\n\
                      # good\n'a,b,c'.split(',')\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"split"
            || call.receiver().is_none()
            || call.block().is_some_and(|b| b.as_block_node().is_none())
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut args = arguments.arguments().iter();
        let (Some(arg), None) = (args.next(), args.next()) else { return };
        let (ignore_case, opening, closing) = if let Some(re) = arg.as_regular_expression_node() {
            (re.is_ignore_case(), re.opening_loc().span(), re.closing_loc().span())
        } else if let Some(re) = arg.as_interpolated_regular_expression_node() {
            (re.is_ignore_case(), re.opening_loc().span(), re.closing_loc().span())
        } else {
            return;
        };
        let span = arg.span();
        let content_span = Span::new(opening.end, closing.start);
        let Ok(content) = std::str::from_utf8(ctx.text(content_span)) else { return };
        if ignore_case || content == " " {
            return;
        }
        let Ok(source) = std::str::from_utf8(ctx.text(span)) else { return };
        if !is_deterministic(source) {
            return;
        }
        let new_argument = format!("\"{}\"", replacement(content));
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, new_argument.into_bytes())],
            },
        );
    }
}
