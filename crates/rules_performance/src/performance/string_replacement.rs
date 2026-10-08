//! `Performance/StringReplacement`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_replacement.rb`.

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `Util::LITERAL_REGEX` with Ruby's ASCII-only `\w` and `\s`.
const LITERAL: &str =
    r#"(?:[A-Za-z0-9_ \t\n\x0B\x0C\r\-,"'!#%&<>=;:`~/]|\\[^AbBdDgGhHkpPRwWXsSzZ0-9])"#;

static DETERMINISTIC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\A{LITERAL}+\z")).expect("static regex"));

/// Identifies places where `gsub` can be replaced by `tr` or `delete`.
#[derive(Debug, Clone)]
pub struct StringReplacement;

/// A parsed first argument: its source text (when it has one) and whether
/// the regexp carried options.
struct First {
    source: Option<Vec<u8>>,
    options: bool,
    is_str: bool,
}

impl Rule for StringReplacement {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringReplacement",
        department: Department::Performance,
        summary: "Use `tr` instead of `gsub` when you are replacing the same number of \
                  characters. Use `delete` instead of `gsub` when you are deleting characters.",
        explanation: "Identifies places where `gsub` can be replaced by `tr` or `delete`.\n\n\
                      ```ruby\n# bad\n'abc'.gsub('b', 'd')\n'abc'.gsub('a', '')\n\
                      'abc'.gsub(/a/, 'd')\n'abc'.gsub!('a', 'd')\n\n# good\n\
                      'abc'.gsub(/.*/, 'a')\n'abc'.gsub(/a+/, 'd')\n'abc'.tr('b', 'd')\n\
                      'a b c'.delete(' ')\n```",
        enabled_by_default: true,
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
        let name = call.name();
        let name = name.as_slice();
        if name != b"gsub" && name != b"gsub!" {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        if arguments.arguments().len() != 2
            || call.block().is_some_and(|b| b.as_block_node().is_none())
        {
            return;
        }
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let first_param = &args[0];
        let Some(second) = args[1].as_string_node() else { return };
        let Some(first) = first_source(first_param, ctx) else { return };
        let second_source = second.unescaped().to_vec();

        // accept_second_param?
        if char_len(&second_source) > 1 {
            return;
        }
        // accept_first_param?
        let Some(raw) = first.source.clone() else { return };
        let first_source = if first.is_str {
            raw
        } else {
            if first.options {
                return;
            }
            let Ok(text) = std::str::from_utf8(&raw) else { return };
            if !DETERMINISTIC_REGEX.is_match(text) {
                return;
            }
            interpret_string_escapes(&raw)
        };
        if char_len(&first_source) != 1 {
            return;
        }

        let delete = second_source.is_empty() && char_len(&first_source) == 1;
        let bang = if name == b"gsub!" { "!" } else { "" };
        let replacement = format!("{}{bang}", if delete { "delete" } else { "tr" });
        let current = String::from_utf8_lossy(name);
        let message = format!("Use `{replacement}` instead of `{current}`.");

        let Some(selector) = call.message_loc() else { return };
        let node_span = call_span_excluding_block(&call);
        let range = Span::new(selector.span().start, node_span.end);

        let mut edits = vec![Edit::replace(selector.span(), replacement.into_bytes())];
        if !first.is_str {
            edits.push(Edit::replace(
                first_param.span(),
                to_string_literal(&first_source).into_bytes(),
            ));
        }
        if delete {
            let end_range = Span::new(first_param.span().end, node_span.end);
            let suffix = call.closing_loc().map_or_else(Vec::new, |l| ctx.text(l.span()).to_vec());
            edits.push(Edit::replace(end_range, suffix));
        }
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

fn char_len(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).chars().count()
}

/// `first_source`: `${regexp str (send (const nil? :Regexp) {:new :compile} _)}`.
fn first_source(node: &Node<'_>, ctx: &Context<'_>) -> Option<First> {
    if let Some(s) = node.as_string_node() {
        return Some(First { source: Some(s.unescaped().to_vec()), options: false, is_str: true });
    }
    if node.as_regular_expression_node().is_some()
        || node.as_interpolated_regular_expression_node().is_some()
    {
        return Some(regex_literal(node, ctx));
    }
    let call = node.as_call_node()?;
    let name = call.name();
    if !matches!(name.as_slice(), b"new" | b"compile") || call.is_safe_navigation() {
        return None;
    }
    let receiver = call.receiver()?;
    let constant = receiver.as_constant_read_node()?;
    if constant.name().as_slice() != b"Regexp" {
        return None;
    }
    let arguments = call.arguments()?;
    if arguments.arguments().len() != 1 || call.block().is_some() {
        return None;
    }
    let arg = arguments.arguments().first()?;
    if let Some(s) = arg.as_string_node() {
        return Some(First { source: Some(s.unescaped().to_vec()), options: false, is_str: false });
    }
    if arg.as_regular_expression_node().is_some()
        || arg.as_interpolated_regular_expression_node().is_some()
    {
        let mut first = regex_literal(&arg, ctx);
        first.is_str = false;
        return Some(first);
    }
    // Any other argument: `source_from_regex_constructor` yields nil.
    Some(First { source: None, options: false, is_str: false })
}

/// `source_from_regex_literal`: `[source, options]`. Interpolated or empty
/// regexps (and flagged ones) never produce an offense, so they only need to
/// be recognisable as non-matching.
fn regex_literal(node: &Node<'_>, ctx: &Context<'_>) -> First {
    let Some(regexp) = node.as_regular_expression_node() else {
        return First { source: None, options: true, is_str: false };
    };
    let options = regexp.closing_loc().span().len() != 1;
    let content = ctx.text(regexp.content_loc().span());
    if content.is_empty() || content.contains(&b'\n') {
        return First { source: None, options, is_str: false };
    }
    First { source: Some(content.to_vec()), options, is_str: false }
}

/// RuboCop's `StringInterpreter.interpret`.
fn interpret_string_escapes(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if src[i] != b'\\' || i + 1 >= src.len() {
            out.push(src[i]);
            i += 1;
            continue;
        }
        let c = src[i + 1];
        let hex = |s: &[u8]| s.iter().take_while(|b| b.is_ascii_hexdigit()).count();
        match c {
            b'a' => (out.push(0x07), i += 2).1,
            b'b' => (out.push(0x08), i += 2).1,
            b'e' => (out.push(0x1B), i += 2).1,
            b'f' => (out.push(0x0C), i += 2).1,
            b'n' => (out.push(b'\n'), i += 2).1,
            b'r' => (out.push(b'\r'), i += 2).1,
            b's' => (out.push(b' '), i += 2).1,
            b't' => (out.push(b'\t'), i += 2).1,
            b'v' => (out.push(0x0B), i += 2).1,
            b'\n' => i += 2,
            b'0'..=b'9' => {
                let n = src[i + 1..].iter().take(3).take_while(|b| b.is_ascii_digit()).count();
                let digits = std::str::from_utf8(&src[i + 1..i + 1 + n]).unwrap_or("0");
                out.push(u32::from_str_radix(digits, 8).map_or(0, |v| (v & 0xFF) as u8));
                i += 1 + n;
            }
            b'x' if hex(&src[i + 2..]).min(2) > 0 => {
                let n = hex(&src[i + 2..]).min(2);
                let digits = std::str::from_utf8(&src[i + 2..i + 2 + n]).unwrap_or("0");
                out.push(u8::from_str_radix(digits, 16).unwrap_or(0));
                i += 2 + n;
            }
            b'u' if src.get(i + 2) == Some(&b'{') => {
                if let Some(close) = src[i + 3..].iter().position(|&b| b == b'}') {
                    let body = String::from_utf8_lossy(&src[i + 3..i + 3 + close]).into_owned();
                    for h in body.split_whitespace() {
                        if let Some(ch) = u32::from_str_radix(h, 16).ok().and_then(char::from_u32) {
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                    }
                    i += 4 + close;
                } else {
                    out.push(b'{');
                    i += 3;
                }
            }
            b'u' if hex(&src[(i + 2).min(src.len())..]) >= 4 => {
                let digits = std::str::from_utf8(&src[i + 2..i + 6]).unwrap_or("0");
                if let Some(ch) = u32::from_str_radix(digits, 16).ok().and_then(char::from_u32) {
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                i += 6;
            }
            _ => {
                // Any other escaped char: the char itself (may be multibyte).
                let rest = &src[i + 1..];
                let len = std::str::from_utf8(rest)
                    .ok()
                    .or_else(|| std::str::from_utf8(&rest[..rest.len().min(4)]).ok())
                    .and_then(|s| s.chars().next())
                    .map_or(1, char::len_utf8);
                out.extend_from_slice(&rest[..len]);
                i += 1 + len;
            }
        }
    }
    out
}

/// RuboCop's `Util#to_string_literal`.
fn to_string_literal(bytes: &[u8]) -> String {
    if needs_escaping(bytes) && std::str::from_utf8(bytes).is_ok() {
        format!("\"{}\"", ruby_inspect_body(bytes))
    } else {
        let s = String::from_utf8_lossy(bytes).replace('\\', "\\\\").replace("\\\"", "\"");
        format!("'{s}'")
    }
}

/// RuboCop's `Util#needs_escaping?`.
fn needs_escaping(bytes: &[u8]) -> bool {
    double_quotes_required(&ruby_inspect_body(bytes).replace("\\\"", "\""))
}

/// RuboCop's `Util#double_quotes_required?`.
fn double_quotes_required(s: &str) -> bool {
    if s.contains('\'') {
        return true;
    }
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            if (i - start) % 2 == 1 && bytes.get(i) != Some(&b'"') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// `String#inspect`'s body (no surrounding quotes).
fn ruby_inspect_body(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.iter().fold(String::new(), |mut out, b| {
            let _ = write!(out, "\\x{b:02X}");
            out
        });
    };
    let mut out = String::with_capacity(text.len() + 2);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '#' if matches!(chars.peek(), Some('{' | '@' | '$')) => out.push_str("\\#"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\x0B' => out.push_str("\\v"),
            '\x0C' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\x1B' => out.push_str("\\e"),
            '\x7F' => out.push_str("\\x7F"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}
