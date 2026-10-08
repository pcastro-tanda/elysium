//! `Style/RedundantHeredocDelimiterQuotes`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_heredoc_delimiter_quotes.rb`.
//!
//! Upstream's `Heredoc` mixin extracts the opening delimiter's type
//! (`<<`/`<<-`/`<<~`) and quoted body via a regex applied to the whole
//! node's own `source` -- which, for a heredoc, the parser gem defines as
//! just the opening line's text (the body and terminator live in separate
//! `heredoc_body`/`heredoc_end` source maps). Prism's `opening_loc` is
//! exactly that same opening-line text for all four heredoc-capable string
//! kinds (`StringNode`, `InterpolatedStringNode`, `XStringNode`,
//! `InterpolatedXStringNode`), so this port parses `opening_loc`'s bytes
//! directly instead of regexing `node.source`. The body/terminator text is
//! simply the source between `opening_loc`'s end and `closing_loc`'s start
//! (body) and `closing_loc` itself (terminator), since Prism gives no
//! separate maps for them.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_heredoc;
use ruby_ast::{LocationExt as _, Node, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Remove the redundant heredoc delimiter quotes, use `%<replacement>s` instead.";

/// Checks for redundant heredoc delimiter quotes.
#[derive(Debug, Clone)]
pub struct RedundantHeredocDelimiterQuotes;

impl Rule for RedundantHeredocDelimiterQuotes {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantHeredocDelimiterQuotes",
        department: Department::Style,
        summary: "Checks for redundant heredoc delimiter quotes.",
        explanation: "\
Checks for redundant heredoc delimiter quotes.

```ruby
# bad
do_something(<<~'EOS')
  no string interpolation style text
EOS

# good
do_something(<<~EOS)
  no string interpolation style text
EOS

do_something(<<~'EOS')
  #{string_interpolation_style_text_not_evaluated}
EOS

do_something(<<~'EOS')
  Preserve \\
  newlines
EOS
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::StringNode,
            NodeKind::InterpolatedStringNode,
            NodeKind::XStringNode,
            NodeKind::InterpolatedXStringNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !is_heredoc(node) {
            return;
        }
        let Some((opening, closing)) = heredoc_locs(node) else { return };
        let open_bytes = ctx.text(opening);
        let Some(parsed) = parse_opening(open_bytes) else { return };
        if need_heredoc_delimiter_quotes(ctx, &parsed, opening, closing) {
            return;
        }

        let mut replacement = parsed.heredoc_type.to_vec();
        replacement.extend_from_slice(parsed.delimiter);
        let message = MSG.replace("%<replacement>s", &String::from_utf8_lossy(&replacement));

        ctx.report_with_fix(
            &Self::META,
            opening,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(opening, replacement.into_boxed_slice())],
            },
        );
    }
}

/// The opening (`<<~"EOS"`) and closing (`EOS\n`) locations shared by all
/// four heredoc-capable string literal kinds.
fn heredoc_locs(node: &Node<'_>) -> Option<(Span, Span)> {
    let (opening, closing) = match node {
        Node::StringNode { .. } => {
            let n = node.as_string_node()?;
            (n.opening_loc()?, n.closing_loc()?)
        }
        Node::InterpolatedStringNode { .. } => {
            let n = node.as_interpolated_string_node()?;
            (n.opening_loc()?, n.closing_loc()?)
        }
        Node::XStringNode { .. } => {
            let n = node.as_x_string_node()?;
            (n.opening_loc(), n.closing_loc())
        }
        Node::InterpolatedXStringNode { .. } => {
            let n = node.as_interpolated_x_string_node()?;
            (n.opening_loc(), n.closing_loc())
        }
        _ => return None,
    };
    Some((opening.span(), closing.span()))
}

/// The opening delimiter's parsed shape: `<<`/`<<-`/`<<~`, the quote
/// character (`'`/`"`, or `0` for bare/backtick-quoted -- backtick is never
/// treated as redundant, matching upstream's `start_with?("'", '"')`), and
/// the delimiter name without its quotes.
struct OpeningDelimiter<'a> {
    heredoc_type: &'a [u8],
    quote: u8,
    delimiter: &'a [u8],
}

fn parse_opening(bytes: &[u8]) -> Option<OpeningDelimiter<'_>> {
    if !bytes.starts_with(b"<<") {
        return None;
    }
    let mut i = 2;
    if bytes.get(i).is_some_and(|&b| b == b'~' || b == b'-') {
        i += 1;
    }
    let heredoc_type = &bytes[..i];
    let rest = &bytes[i..];
    if rest.len() >= 2 && (rest[0] == b'\'' || rest[0] == b'"') && rest[rest.len() - 1] == rest[0] {
        Some(OpeningDelimiter { heredoc_type, quote: rest[0], delimiter: &rest[1..rest.len() - 1] })
    } else {
        Some(OpeningDelimiter { heredoc_type, quote: 0, delimiter: rest })
    }
}

/// RuboCop's `need_heredoc_delimiter_quotes?`.
fn need_heredoc_delimiter_quotes(
    ctx: &Context<'_>,
    parsed: &OpeningDelimiter<'_>,
    opening: Span,
    closing: Span,
) -> bool {
    if parsed.quote == 0 {
        return true;
    }

    let end_stripped = trim_ruby_whitespace(ctx.text(closing));
    if !end_stripped.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'_') {
        return true;
    }

    parsed.quote == b'\''
        && contains_interpolation_or_escape(ctx.text(Span::new(opening.end, closing.start)))
}

/// `STRING_INTERPOLATION_OR_ESCAPED_CHARACTER_PATTERN = /#(\{|@|\$)|\\/`.
fn contains_interpolation_or_escape(body: &[u8]) -> bool {
    let mut i = 0;
    while i < body.len() {
        match body[i] {
            b'\\' => return true,
            b'#' if matches!(body.get(i + 1), Some(b'{' | b'@' | b'$')) => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// Ruby's `String#strip`, applied to raw bytes.
fn trim_ruby_whitespace(bytes: &[u8]) -> &[u8] {
    let is_ws = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C | 0);
    let start = bytes.iter().position(|&b| !is_ws(b)).unwrap_or(bytes.len());
    let end = bytes.iter().rposition(|&b| !is_ws(b)).map_or(start, |i| i + 1);
    &bytes[start..end]
}
