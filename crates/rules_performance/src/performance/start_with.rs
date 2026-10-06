//! `Performance/StartWith`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/start_with.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str =
    "Use `String#start_with?` instead of a regex match anchored to the beginning of the string.";

/// `Util::LITERAL_REGEX`'s single-character alternative:
/// `[\w\s\-,"'!#%&<>=;:~/` and backtick`]`.
fn is_safe_literal_char(c: char) -> bool {
    c.is_alphanumeric()
        || c == '_'
        || matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C' | '\x0B')
        || matches!(
            c,
            '-' | ',' | '"' | '\'' | '!' | '#' | '%' | '&' | '<' | '>' | '=' | ';' | ':' | '`'
                | '~' | '/'
        )
}

/// `Util::LITERAL_REGEX`'s escaped alternative: `\\[^AbBdDgGhHkpPRwWXsSzZ0-9]`.
fn is_safe_escaped_char(c: char) -> bool {
    !matches!(
        c,
        'A' | 'b'
            | 'B'
            | 'd'
            | 'D'
            | 'g'
            | 'G'
            | 'h'
            | 'H'
            | 'k'
            | 'p'
            | 'P'
            | 'R'
            | 'w'
            | 'W'
            | 'X'
            | 's'
            | 'S'
            | 'z'
            | 'Z'
            | '0'..='9'
    )
}

/// `/\A(?:#{LITERAL_REGEX})+\z/` against `s`.
fn all_literal(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(n) if is_safe_escaped_char(n) => {}
                _ => return false,
            }
        } else if !is_safe_literal_char(c) {
            return false;
        }
    }
    true
}

/// `literal_at_start?`.
fn literal_at_start(s: &str, safe_multiline: bool) -> bool {
    if let Some(rest) = s.strip_prefix("\\A") {
        if all_literal(rest) {
            return true;
        }
    }
    if !safe_multiline {
        if let Some(rest) = s.strip_prefix('^') {
            return all_literal(rest);
        }
    }
    false
}

fn hex_val(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |acc, b| acc * 16 + char::from(*b).to_digit(16).unwrap_or(0))
}

/// `StringInterpreter.interpret`.
fn interpret_string_escapes(text: &str) -> Vec<u8> {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 >= b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let c = b[i + 1];
        i += 2;
        match c {
            b'a' => out.push(0x07),
            b'b' => out.push(0x08),
            b'e' => out.push(0x1B),
            b'f' => out.push(0x0C),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b's' => out.push(b' '),
            b't' => out.push(b'\t'),
            b'v' => out.push(0x0B),
            b'\n' => {}
            b'0'..=b'9' => {
                let mut end = i;
                while end < b.len() && end < i + 2 && b[end].is_ascii_digit() {
                    end += 1;
                }
                let digits = &b[i - 1..end];
                i = end;
                let value = digits.iter().fold(0u32, |acc, d| acc * 8 + u32::from(d - b'0'));
                #[allow(clippy::cast_possible_truncation)]
                out.push(value as u8);
            }
            b'x' if i < b.len() && b[i].is_ascii_hexdigit() => {
                let mut end = i + 1;
                if end < b.len() && b[end].is_ascii_hexdigit() {
                    end += 1;
                }
                #[allow(clippy::cast_possible_truncation)]
                out.push(hex_val(&b[i..end]) as u8);
                i = end;
            }
            b'u' if i < b.len() && b[i] == b'{' => {
                if let Some(close) = b[i..].iter().position(|&x| x == b'}') {
                    let inner = String::from_utf8_lossy(&b[i + 1..i + close]).into_owned();
                    for part in inner.split_whitespace() {
                        let value = hex_val(part.as_bytes());
                        if let Some(ch) = char::from_u32(value) {
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                    }
                    i += close + 1;
                } else {
                    out.push(b'u');
                }
            }
            b'u' if i + 4 <= b.len() && b[i..i + 4].iter().all(u8::is_ascii_hexdigit) => {
                if let Some(ch) = char::from_u32(hex_val(&b[i..i + 4])) {
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                i += 4;
            }
            _ => {
                // Any other escaped char (possibly multibyte): keep it literally.
                out.push(c);
            }
        }
    }
    out
}

/// `String#inspect`'s body for valid UTF-8 text.
fn inspect_body(text: &str) -> String {
    use std::fmt::Write as _;
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
            if (i - start) % 2 == 1 && !matches!(bytes.get(i), Some(b'\\' | b'"')) {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// `Util#to_string_literal`.
fn to_string_literal(bytes: &[u8]) -> Vec<u8> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        let body = inspect_body(text);
        if double_quotes_required(&body.replace("\\\"", "\"")) {
            return format!("\"{body}\"").into_bytes();
        }
    }
    let mut out = vec![b'\''];
    let mut escaped = Vec::with_capacity(bytes.len());
    for &b in bytes {
        escaped.push(b);
        if b == b'\\' {
            escaped.push(b'\\');
        }
    }
    let mut j = 0;
    while j < escaped.len() {
        if escaped[j] == b'\\' && escaped.get(j + 1) == Some(&b'"') {
            j += 1;
            continue;
        }
        out.push(escaped[j]);
        j += 1;
    }
    out.push(b'\'');
    out
}

/// Identifies unnecessary use of a regex where `String#start_with?` would suffice.
#[derive(Debug, Clone)]
pub struct StartWith {
    safe_multiline: bool,
}

impl StartWith {
    /// `(regexp (str $#literal_at_start?) (regopt))`: the regexp's raw source.
    fn anchored_regexp<'a>(&self, node: &Node<'_>, ctx: &'a Context<'_>) -> Option<&'a str> {
        let re = node.as_regular_expression_node()?;
        if re.is_ignore_case()
            || re.is_extended()
            || re.is_multi_line()
            || re.is_once()
            || re.is_euc_jp()
            || re.is_ascii_8bit()
            || re.is_windows_31j()
            || re.is_utf_8()
        {
            return None;
        }
        let text = std::str::from_utf8(ctx.text(re.content_loc().span())).ok()?;
        literal_at_start(text, self.safe_multiline).then_some(text)
    }
}

impl Rule for StartWith {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StartWith",
        department: Department::Performance,
        summary: "Use `start_with?` instead of a regex match anchored to the beginning of a string.",
        explanation: "Identifies unnecessary use of a regex where `String#start_with?` would suffice.\n\nThis cop has a `SafeMultiline` option, `true` by default, because `^start` behaves differently from `start_with?` for multiline receivers.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "SafeMultiline",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Do not flag `^` anchored regexps.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { safe_multiline: options.bool("SafeMultiline") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if !matches!(name, b"match" | b"=~" | b"match?") {
            return;
        }
        if let Some(block) = call.block() {
            if block.as_block_node().is_none() {
                return;
            }
        }
        let Some(args) = call.arguments() else { return };
        let args = args.arguments();
        let mut it = args.iter();
        let (Some(arg), None) = (it.next(), it.next()) else { return };

        // (receiver, regex source, receiver node span)
        let found = if let Some(recv) = call.receiver() {
            if let Some(re) = self.anchored_regexp(&arg, ctx) {
                Some((recv.span(), re))
            } else if !call.is_safe_navigation() {
                self.anchored_regexp(&recv, ctx).map(|re| (arg.span(), re))
            } else {
                None
            }
        } else {
            None
        };
        let Some((receiver_span, regex_str)) = found else { return };

        let span = call_span_excluding_block(&call);
        let stripped = regex_str.strip_prefix("\\A").unwrap_or(&regex_str[1..]);
        let literal = to_string_literal(&interpret_string_escapes(stripped));
        let dot = call.call_operator_loc().map_or(&b"."[..], |l| ctx.text(l.span()));
        let mut new_source = ctx.text(receiver_span).to_vec();
        new_source.extend_from_slice(dot);
        new_source.extend_from_slice(b"start_with?(");
        new_source.extend_from_slice(&literal);
        new_source.push(b')');
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Unsafe, edits: vec![Edit::replace(span, new_source)] },
        );
    }
}
