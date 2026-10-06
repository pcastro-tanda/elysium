//! `Performance/Squeeze`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/squeeze.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Use `squeeze('a')` instead of `gsub(/a+/, 'a')`.
#[derive(Debug, Clone)]
pub struct Squeeze;

/// `Util::LITERAL_REGEX` single-character alternative:
/// `[\w\s\-,"'!#%&<>=;:~/]` plus a backtick.
fn is_literal_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '_' | ' '
                | '\t'
                | '\r'
                | '\n'
                | '\u{b}'
                | '\u{c}'
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

/// `repeating_literal?` plus `regexp_str[0..-2]` and `interpret_string_escapes`:
/// when `source` is `\A(?:LITERAL_REGEX)\+\z`, the interpreted literal.
fn repeated_literal(source: &str) -> Option<String> {
    let body = source.strip_suffix('+')?;
    let mut chars = body.chars();
    let first = chars.next()?;
    if first != '\\' {
        return (chars.next().is_none() && is_literal_char(first)).then(|| first.to_string());
    }
    let escaped = chars.next()?;
    if chars.next().is_some()
        || escaped.is_ascii_digit()
        || "AbBdDgGhHkpPRwWXsSzZ".contains(escaped)
    {
        return None;
    }
    Some(match escaped {
        'a' => "\u{7}".to_string(),
        'e' => "\u{1b}".to_string(),
        'f' => "\u{c}".to_string(),
        'n' => "\n".to_string(),
        'r' => "\r".to_string(),
        't' => "\t".to_string(),
        'v' => "\u{b}".to_string(),
        '\n' => String::new(),
        other => other.to_string(),
    })
}

/// `Util#to_string_literal` for a single-character string.
fn to_string_literal(text: &str) -> String {
    let mut chars = text.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return format!("'{}'", text.replace('\\', "\\\\"));
    };
    let escaped = match c {
        '\n' => Some("\\n".to_string()),
        '\t' => Some("\\t".to_string()),
        '\r' => Some("\\r".to_string()),
        '\u{c}' => Some("\\f".to_string()),
        '\u{b}' => Some("\\v".to_string()),
        '\u{1b}' => Some("\\e".to_string()),
        '\u{7}' => Some("\\a".to_string()),
        '\u{8}' => Some("\\b".to_string()),
        '\u{7f}' => Some("\\x7F".to_string()),
        c if c.is_control() && (c as u32) < 0x20 => Some(format!("\\u{:04X}", c as u32)),
        _ => None,
    };
    if let Some(escaped) = escaped {
        return format!("\"{escaped}\"");
    }
    match c {
        '\'' => "\"'\"".to_string(),
        '\\' => "'\\\\'".to_string(),
        c => format!("'{c}'"),
    }
}

impl Rule for Squeeze {
    const META: RuleMeta = RuleMeta {
        name: "Performance/Squeeze",
        department: Department::Performance,
        summary: "Use `squeeze('a')` instead of `gsub(/a+/, 'a')`.",
        explanation: "\
Identifies places where `gsub(/a+/, 'a')` and `gsub!(/a+/, 'a')`
can be replaced by `squeeze('a')` and `squeeze!('a')`.

The `squeeze('a')` method is faster than `gsub(/a+/, 'a')`.

```ruby
# bad
str.gsub(/a+/, 'a')
str.gsub!(/a+/, 'a')

# good
str.squeeze('a')
str.squeeze!('a')
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let (bad, good) = match name.as_slice() {
            b"gsub" => ("gsub", "squeeze"),
            b"gsub!" => ("gsub!", "squeeze!"),
            _ => return,
        };
        let Some(receiver) = call.receiver() else { return };
        let Some(args) = call.arguments() else { return };
        let list = args.arguments();
        if list.len() != 2 {
            return;
        }
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let mut iter = list.iter();
        let (Some(first), Some(second)) = (iter.next(), iter.next()) else { return };
        let Some(regexp) = first.as_regular_expression_node() else { return };
        let Some(replacement) = second.as_string_node() else { return };
        if regexp.closing_loc().span().len() != 1 {
            return;
        }
        let Ok(source) = std::str::from_utf8(ctx.text(regexp.content_loc().span())) else {
            return;
        };
        let Some(literal) = repeated_literal(source) else { return };
        if replacement.unescaped() != literal.as_bytes() {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let Some(dot) = call.call_operator_loc() else { return };

        let end = call.closing_loc().map_or_else(|| second.span().end, |c| c.span().end);
        let whole = Span::new(node.span().start, end);
        let new_code = format!(
            "{}{}{}({})",
            String::from_utf8_lossy(ctx.text(receiver.span())),
            String::from_utf8_lossy(ctx.text(dot.span())),
            good,
            to_string_literal(&literal),
        );
        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            format!("Use `{good}` instead of `{bad}`."),
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(whole, new_code.into_bytes())],
            },
        );
    }
}
