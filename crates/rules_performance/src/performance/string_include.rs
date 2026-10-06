//! `Performance/StringInclude`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_include.rb`.
//!
//! `match-with-lvasgn` (`/(?<x>lit)/ =~ str`) cannot occur here: a regexp
//! with a named capture is never literal-only.

use std::fmt::Write as _;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Use `String#include?` instead of a regex match with literal-only pattern.
#[derive(Debug, Clone)]
pub struct StringInclude;

impl Rule for StringInclude {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringInclude",
        department: Department::Performance,
        summary: "Use `String#include?` instead of a regex match with literal-only pattern.",
        explanation: "Identifies unnecessary use of a regex where `String#include?` would \
                      suffice.\n\n```ruby\n# bad\nstr.match?(/ab/)\n/ab/.match?(str)\n\
                      str =~ /ab/\n/ab/ =~ str\nstr.match(/ab/)\n/ab/.match(str)\n\
                      /ab/ === str\n\n# good\nstr.include?('ab')\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if !matches!(name, b"match" | b"=~" | b"!~" | b"match?" | b"===") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [arg] = args.as_slice() else { return };

        // `(call $!nil? {:match :=~ :!~ :match?} (regexp (str $lit) (regopt)))`
        // `(send (regexp ...) {:match :match? :===} $_)` / `(send (regexp ...) :=~ $_)`
        let (regex_source, other) = if name != b"===" && literal_regexp(arg, ctx).is_some() {
            (literal_regexp(arg, ctx), &receiver)
        } else if !call.is_safe_navigation()
            && matches!(name, b"match" | b"match?" | b"===" | b"=~")
            && literal_regexp(&receiver, ctx).is_some()
        {
            (literal_regexp(&receiver, ctx), arg)
        } else {
            return;
        };
        let Some(regex_source) = regex_source else { return };
        let other_source = String::from_utf8_lossy(ctx.text(other.span())).into_owned();

        let negation = !call.is_safe_navigation() && name == b"!~";
        let neg = if negation { "!" } else { "" };
        let message = format!(
            "Use `{neg}String#include?` instead of a regex match with literal-only pattern."
        );

        let span = call_span_excluding_block(&call);
        let literal = to_string_literal(&interpret_string_escapes(&regex_source));
        let dot = call.call_operator_loc().map_or(&b"."[..], |l| ctx.text(l.span()));
        let new_source =
            format!("{neg}{other_source}{}include?({literal})", String::from_utf8_lossy(dot));
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, new_source.into_bytes())],
            },
        );
    }
}

/// `(regexp (str $#literal?) (regopt))`: the raw source of a flag-less regexp
/// literal made only of `LITERAL_REGEX` characters.
fn literal_regexp(node: &Node<'_>, ctx: &Context<'_>) -> Option<String> {
    let regexp = node.as_regular_expression_node()?;
    if regexp.is_ignore_case()
        || regexp.is_extended()
        || regexp.is_multi_line()
        || regexp.is_once()
        || regexp.is_euc_jp()
        || regexp.is_ascii_8bit()
        || regexp.is_windows_31j()
        || regexp.is_utf_8()
    {
        return None;
    }
    let content = regexp.content_loc().span();
    let content = std::str::from_utf8(ctx.text(content)).ok()?;
    is_literal(content).then(|| content.to_string())
}

/// `/\A#{Util::LITERAL_REGEX}+\z/`.
fn is_literal(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) if !"AbBdDgGhHkpPRwWXsSzZ0123456789".contains(next) => {}
                _ => return false,
            }
        } else if !(c.is_ascii_alphanumeric()
            || c == '_'
            || matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C')
            || "-,\"'!#%&<>=;:`~/".contains(c))
        {
            return false;
        }
    }
    true
}

/// `RuboCop::StringInterpreter.interpret`.
#[allow(clippy::many_single_char_names)]
fn interpret_string_escapes(s: &str) -> Vec<u8> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::with_capacity(s.len());
    let mut buf = [0u8; 4];
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' || i + 1 >= chars.len() {
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            i += 1;
            continue;
        }
        let e = chars[i + 1];
        i += 2;
        match e {
            'a' => out.push(0x07),
            'b' => out.push(0x08),
            'e' => out.push(0x1B),
            'f' => out.push(0x0C),
            'n' => out.push(b'\n'),
            'r' => out.push(b'\r'),
            's' => out.push(b' '),
            't' => out.push(b'\t'),
            'v' => out.push(0x0B),
            '\n' => {}
            '0'..='9' => {
                let mut value = 0u32;
                let mut n = 0;
                // `\d{1,3}`; `String#to_i(8)` stops at the first non-octal digit.
                let mut octal = true;
                let mut j = i - 1;
                while n < 3 && j < chars.len() && chars[j].is_ascii_digit() {
                    if octal && chars[j] < '8' {
                        value = value * 8 + chars[j].to_digit(8).unwrap_or(0);
                    } else {
                        octal = false;
                    }
                    n += 1;
                    j += 1;
                }
                i = j;
                out.push((value & 0xFF) as u8);
            }
            'x' if chars.get(i).is_some_and(char::is_ascii_hexdigit) => {
                let mut value = 0u32;
                let mut n = 0;
                while n < 2 && chars.get(i).is_some_and(char::is_ascii_hexdigit) {
                    value = value * 16 + chars[i].to_digit(16).unwrap_or(0);
                    i += 1;
                    n += 1;
                }
                out.push((value & 0xFF) as u8);
            }
            'u' if chars.len() >= i + 4 && chars[i..i + 4].iter().all(char::is_ascii_hexdigit) => {
                let value = chars[i..i + 4]
                    .iter()
                    .fold(0u32, |acc, d| acc * 16 + d.to_digit(16).unwrap_or(0));
                i += 4;
                if let Some(ch) = char::from_u32(value) {
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
            }
            'u' if chars.get(i) == Some(&'{') && chars[i..].contains(&'}') => {
                let close = i + chars[i..].iter().position(|&c| c == '}').unwrap_or(0);
                let body: String = chars[i + 1..close].iter().collect();
                for word in body.split_whitespace() {
                    let value = u32::from_str_radix(word, 16).unwrap_or(0);
                    if let Some(ch) = char::from_u32(value) {
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                }
                i = close + 1;
            }
            other => out.extend_from_slice(other.encode_utf8(&mut buf).as_bytes()),
        }
    }
    out
}

/// `Util#to_string_literal`.
fn to_string_literal(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) if double_quotes_required(&inspect_body(text).replace("\\\"", "\"")) => {
            format!("\"{}\"", inspect_body(text))
        }
        _ => {
            let text = String::from_utf8_lossy(bytes);
            format!("'{}'", text.replace('\\', "\\\\").replace("\\\"", "\""))
        }
    }
}

/// `Util#double_quotes_required?`.
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

/// `String#inspect` without the surrounding quotes.
fn inspect_body(text: &str) -> String {
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
                let _ = write!(out, "\\x{:02X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}
