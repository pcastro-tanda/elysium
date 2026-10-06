//! `Performance/DeleteSuffix`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/delete_suffix.rb` (and its
//! `RegexpMetacharacter` mixin).

use std::fmt::Write as _;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, is_heredoc};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Use `delete_suffix` instead of `gsub`.
#[derive(Debug, Clone)]
pub struct DeleteSuffix {
    safe_multiline: bool,
    enabled_for_target: bool,
}

impl Rule for DeleteSuffix {
    const META: RuleMeta = RuleMeta {
        name: "Performance/DeleteSuffix",
        department: Department::Performance,
        summary: "Use `delete_suffix` instead of `gsub`.",
        explanation: "Identifies places where `gsub(/suffix\\z/, '')` and `sub(/suffix\\z/, '')` can be replaced by `delete_suffix('suffix')`. With `SafeMultiline: false`, `suffix$` anchors are flagged too. Unsafe because `Pathname` has `sub` but not `delete_suffix`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "SafeMultiline",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Only flag `\\z` anchors, not `$`, since `$` matches at every line end.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            safe_multiline: options.bool("SafeMultiline"),
            enabled_for_target: options.target_ruby_version() >= 2.5,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled_for_target {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let bad_method = call.name();
        let bad_method = bad_method.as_slice();
        let good_method = match bad_method {
            b"gsub" | b"sub" => "delete_suffix",
            b"gsub!" | b"sub!" => "delete_suffix!",
            _ => return,
        };
        let Some(receiver) = call.receiver() else { return };
        if call.block().is_some_and(|b| b.kind() == NodeKind::BlockArgumentNode) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut it = arguments.arguments().iter();
        let (Some(pattern), Some(replacement), None) = (it.next(), it.next(), it.next()) else {
            return;
        };
        let Some(regexp) = pattern.as_regular_expression_node() else { return };
        // `(regopt)` with no options.
        if regexp.closing_loc().span().len() != 1 {
            return;
        }
        let content = ctx.text(regexp.content_loc().span());
        // A multi-line regexp is several `str` children upstream.
        if content.is_empty() || content.contains(&b'\n') {
            return;
        }
        let Some(string) = replacement.as_string_node() else { return };
        if is_heredoc(&replacement) || !string.unescaped().is_empty() {
            return;
        }
        let content = String::from_utf8_lossy(content).into_owned();
        if !self.literal_at_end(&content) {
            return;
        }

        let message =
            format!("Use `{good_method}` instead of `{}`.", String::from_utf8_lossy(bad_method));
        let Some(selector) = call.message_loc() else { return };

        let regexp_str = drop_end_metacharacter(&content);
        let regexp_str = interpret_string_escapes(&regexp_str);
        let string_literal = to_string_literal(&regexp_str);

        let span = call_span_excluding_block(&call);
        let dot = call.call_operator_loc().map_or(&b""[..], |loc| ctx.text(loc.span()));
        let mut new_code = Vec::new();
        new_code.extend_from_slice(ctx.text(receiver.span()));
        new_code.extend_from_slice(dot);
        new_code.extend_from_slice(good_method.as_bytes());
        new_code.push(b'(');
        new_code.extend_from_slice(&string_literal);
        new_code.push(b')');

        ctx.report_with_fix(
            &Self::META,
            selector.span(),
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, new_code)],
            },
        );
    }
}

impl DeleteSuffix {
    /// `RegexpMetacharacter#literal_at_end?`.
    fn literal_at_end(&self, regexp: &str) -> bool {
        if let Some(body) = regexp.strip_suffix("\\z") {
            if all_literal(body) {
                return true;
            }
        }
        !self.safe_multiline && regexp.strip_suffix('$').is_some_and(all_literal)
    }
}

/// `/\A(?:#{Util::LITERAL_REGEX})+\z/`: one or more literal units.
fn all_literal(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let Some(next) = chars.next() else { return false };
            if matches!(
                next,
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
            ) {
                return false;
            }
        } else if !(c.is_ascii_alphanumeric()
            || c == '_'
            || matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C' | '\x0B')
            || matches!(
                c,
                '-' | ','
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
            ))
        {
            return false;
        }
    }
    true
}

/// `RegexpMetacharacter#drop_end_metacharacter`.
fn drop_end_metacharacter(s: &str) -> String {
    if let Some(stripped) = s.strip_suffix("\\z") {
        stripped.to_string()
    } else {
        let mut chars = s.chars();
        chars.next_back();
        chars.as_str().to_string()
    }
}

/// `RuboCop::StringInterpreter.interpret`: convert the escapes as Ruby would
/// in a double-quoted string literal.
fn interpret_string_escapes(s: &str) -> Vec<u8> {
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let push_char = |out: &mut Vec<u8>, c: char| {
        let mut buf = [0u8; 4];
        out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' || i + 1 >= chars.len() {
            push_char(&mut out, c);
            i += 1;
            continue;
        }
        let next = chars[i + 1];
        match next {
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
                let digits: Vec<char> = chars[i + 1..]
                    .iter()
                    .take(3)
                    .take_while(|d| d.is_ascii_digit())
                    .copied()
                    .collect();
                let octal: u32 =
                    digits.iter().map_while(|d| d.to_digit(8)).fold(0, |acc, d| acc * 8 + d);
                out.push(u8::try_from(octal % 256).unwrap_or(0));
                i += 1 + digits.len();
                continue;
            }
            'x' if chars.get(i + 2).is_some_and(char::is_ascii_hexdigit) => {
                let digits: Vec<char> = chars[i + 2..]
                    .iter()
                    .take(2)
                    .take_while(|d| d.is_ascii_hexdigit())
                    .copied()
                    .collect();
                let value =
                    digits.iter().filter_map(|d| d.to_digit(16)).fold(0, |acc, d| acc * 16 + d);
                out.push(u8::try_from(value).unwrap_or(0));
                i += 2 + digits.len();
                continue;
            }
            'u' if chars.get(i + 2) == Some(&'{') && chars[i + 3..].contains(&'}') => {
                let end = i + 3 + chars[i + 3..].iter().position(|&d| d == '}').unwrap_or(0);
                let body: String = chars[i + 3..end].iter().collect();
                for hex in body.split_whitespace() {
                    if let Some(ch) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                        push_char(&mut out, ch);
                    }
                }
                i = end + 1;
                continue;
            }
            'u' if chars.len() >= i + 6
                && chars[i + 2..i + 6].iter().all(char::is_ascii_hexdigit) =>
            {
                let hex: String = chars[i + 2..i + 6].iter().collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    push_char(&mut out, ch);
                }
                i += 6;
                continue;
            }
            other => push_char(&mut out, other),
        }
        i += 2;
    }
    out
}

/// `Util#to_string_literal`.
fn to_string_literal(bytes: &[u8]) -> Vec<u8> {
    let inspected = inspect_body(bytes);
    if double_quotes_required(&inspected.replace("\\\"", "\"")) {
        format!("\"{inspected}\"").into_bytes()
    } else {
        // In single-quoted strings, double quotes don't need to be escaped.
        let mut doubled = Vec::with_capacity(bytes.len() + 2);
        for &b in bytes {
            doubled.push(b);
            if b == b'\\' {
                doubled.push(b'\\');
            }
        }
        let mut out = vec![b'\''];
        let mut i = 0;
        while i < doubled.len() {
            if doubled[i] == b'\\' && doubled.get(i + 1) == Some(&b'"') {
                out.push(b'"');
                i += 2;
            } else {
                out.push(doubled[i]);
                i += 1;
            }
        }
        out.push(b'\'');
        out
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
            if (i - start) % 2 == 1 && !matches!(bytes.get(i), Some(b'\\' | b'"')) {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// `String#inspect`'s body (no surrounding quotes) for a UTF-8 string;
/// invalid bytes render as `\xHH`.
fn inspect_body(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() + 2);
    for chunk in bytes.utf8_chunks() {
        let mut chars = chunk.valid().chars().peekable();
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
        for b in chunk.invalid() {
            let _ = write!(out, "\\x{b:02X}");
        }
    }
    out
}
