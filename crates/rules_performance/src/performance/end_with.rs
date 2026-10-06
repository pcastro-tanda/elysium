//! `Performance/EndWith`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/end_with.rb`.

use std::sync::LazyLock;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str =
    "Use `String#end_with?` instead of a regex match anchored to the end of the string.";

/// `Util::LITERAL_REGEX` with Ruby's ASCII-only `\w` and `\s`.
const LITERAL: &str =
    r#"(?:[A-Za-z0-9_ \t\n\x0B\x0C\r\-,"'!#%&<>=;:`~/]|\\[^AbBdDgGhHkpPRwWXsSzZ0-9])"#;

static AT_END_BACKSLASH_Z: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\A{LITERAL}+\\z\z")).expect("static regex"));
static AT_END_DOLLAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\A{LITERAL}+\$\z")).expect("static regex"));

/// Use `end_with?` instead of a regex match anchored to the end of a string.
#[derive(Debug, Clone)]
pub struct EndWith {
    safe_multiline: bool,
}

impl Rule for EndWith {
    const META: RuleMeta = RuleMeta {
        name: "Performance/EndWith",
        department: Department::Performance,
        summary: "Use `end_with?` instead of a regex match anchored to the end of a string.",
        explanation: "Identifies unnecessary use of a regex where `String#end_with?` would \
                      suffice.\n\nThis cop has `SafeMultiline` configuration option that `true` \
                      by default because `end$` is unsafe as it will behave incompatible with \
                      `end_with?` for receiver is multiline string.\n\n```ruby\n# bad\n\
                      'abc'.match?(/bc\\Z/)\n/bc\\Z/.match?('abc')\n'abc' =~ /bc\\Z/\n\
                      /bc\\Z/ =~ 'abc'\n'abc'.match(/bc\\Z/)\n/bc\\Z/.match('abc')\n\n# good\n\
                      'abc'.end_with?('bc')\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "SafeMultiline",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "When true, `$` anchored regexps are not flagged.",
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
        let Some(arguments) = call.arguments() else { return };
        if arguments.arguments().len() != 1
            || call.block().is_some_and(|b| b.as_block_node().is_none())
        {
            return;
        }
        let Some(arg) = arguments.arguments().first() else { return };
        let Some(receiver) = call.receiver() else { return };

        // `redundant_regex?`: (receiver node, regexp source) in either order.
        let (receiver_node, regex_src) = if let Some(src) = self.anchored_literal(&arg, ctx) {
            (receiver, src)
        } else if !call.is_safe_navigation() {
            match self.anchored_literal(&receiver, ctx) {
                Some(src) => (arg, src),
                None => return,
            }
        } else {
            return;
        };

        let span = call_span_excluding_block(&call);
        let mut s = String::from_utf8_lossy(&regex_src).into_owned();
        // drop_end_metacharacter
        if let Some(stripped) = s.strip_suffix("\\z") {
            s = stripped.to_owned();
        } else {
            s.pop();
        }
        let interpreted = interpret_string_escapes(s.as_bytes());
        let dot =
            call.call_operator_loc().map_or_else(|| b".".to_vec(), |l| ctx.text(l.span()).to_vec());
        let mut new_source = ctx.text(receiver_node.span()).to_vec();
        new_source.extend_from_slice(&dot);
        new_source.extend_from_slice(b"end_with?(");
        new_source.extend_from_slice(to_string_literal(&interpreted).as_bytes());
        new_source.push(b')');
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, new_source)],
            },
        );
    }
}

impl EndWith {
    /// `(regexp (str $#literal_at_end?) (regopt))`: the regexp source.
    fn anchored_literal(&self, node: &Node<'_>, ctx: &Context<'_>) -> Option<Vec<u8>> {
        let regexp = node.as_regular_expression_node()?;
        if regexp.closing_loc().span().len() != 1 {
            return None;
        }
        let content = ctx.text(regexp.content_loc().span());
        if content.contains(&b'\n') {
            return None;
        }
        let text = std::str::from_utf8(content).ok()?;
        let hit = AT_END_BACKSLASH_Z.is_match(text)
            || (!self.safe_multiline && AT_END_DOLLAR.is_match(text));
        hit.then(|| content.to_vec())
    }
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
