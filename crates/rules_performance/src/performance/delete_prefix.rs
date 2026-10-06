//! `Performance/DeletePrefix`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/delete_prefix.rb` plus the
//! `RegexpMetacharacter` mixin, `Util#to_string_literal` and
//! `StringInterpreter` it relies on.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::fmt::Write as _;

/// Use `delete_prefix` instead of `gsub`.
#[derive(Debug, Clone)]
pub struct DeletePrefix {
    safe_multiline: bool,
    enabled_for_ruby: bool,
}

/// `Util::LITERAL_REGEX`'s single-character alternative.
fn is_literal_char(c: char) -> bool {
    c.is_alphanumeric()
        || c == '_'
        || matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r')
        || matches!(
            c,
            '-' | ',' | '"' | '\'' | '!' | '#' | '%' | '&' | '<' | '>' | '=' | ';' | ':' | '`'
                | '~' | '/'
        )
}

/// `/\A(?:LITERAL_REGEX)+\z/` over the text following the anchor.
fn all_literal(rest: &str) -> bool {
    if rest.is_empty() {
        return false;
    }
    let mut chars = rest.chars();
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
        } else if !is_literal_char(c) {
            return false;
        }
    }
    true
}

/// `literal_at_start?`
fn literal_at_start(regexp: &str, safe_multiline: bool) -> bool {
    if let Some(rest) = regexp.strip_prefix("\\A") {
        if all_literal(rest) {
            return true;
        }
    }
    !safe_multiline && regexp.strip_prefix('^').is_some_and(all_literal)
}

/// `drop_start_metacharacter`
fn drop_start_metacharacter(regexp: &str) -> &str {
    regexp.strip_prefix("\\A").unwrap_or_else(|| &regexp[regexp.chars().next().map_or(0, char::len_utf8)..])
}

/// `StringInterpreter.interpret`: convert escapes as a double-quoted literal.
fn interpret_string_escapes(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' || i + 1 >= chars.len() {
            out.push(c);
            i += 1;
            continue;
        }
        let e = chars[i + 1];
        i += 2;
        match e {
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            'e' => out.push('\x1b'),
            'f' => out.push('\x0c'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            's' => out.push(' '),
            't' => out.push('\t'),
            'v' => out.push('\x0b'),
            '\n' => {}
            '0'..='7' => {
                let mut value = e.to_digit(8).expect("octal digit");
                let mut n = 1;
                while n < 3 && i < chars.len() && chars[i].is_ascii_digit() {
                    value = value * 8 + chars[i].to_digit(10).unwrap_or(0);
                    i += 1;
                    n += 1;
                }
                out.push(char::from_u32(value & 0xff).unwrap_or('\0'));
            }
            '8' | '9' => out.push(e),
            'x' if i < chars.len() && chars[i].is_ascii_hexdigit() => {
                let mut value = 0;
                let mut n = 0;
                while n < 2 && i < chars.len() && chars[i].is_ascii_hexdigit() {
                    value = value * 16 + chars[i].to_digit(16).unwrap_or(0);
                    i += 1;
                    n += 1;
                }
                out.push(char::from_u32(value).unwrap_or('\0'));
            }
            'u' if chars.get(i) == Some(&'{') => {
                let end = chars[i..].iter().position(|&ch| ch == '}').map_or(chars.len(), |p| i + p);
                let body: String = chars[i + 1..end].iter().collect();
                for hex in body.split_whitespace() {
                    if let Some(ch) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
                i = (end + 1).min(chars.len());
            }
            'u' if i + 4 <= chars.len() && chars[i..i + 4].iter().all(char::is_ascii_hexdigit) => {
                let hex: String = chars[i..i + 4].iter().collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
                i += 4;
            }
            other => out.push(other),
        }
    }
    out
}

/// `String#inspect` without the surrounding quotes (UTF-8, printable kept).
fn inspect_body(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            '\x0b' => out.push_str("\\v"),
            '\x1b' => out.push_str("\\e"),
            '#' if matches!(chars.get(i + 1), Some('{' | '@' | '$')) => out.push_str("\\#"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// `Util#double_quotes_required?`
fn double_quotes_required(s: &str) -> bool {
    if s.contains('\'') {
        return true;
    }
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            let start = i;
            while i < b.len() && b[i] == b'\\' {
                i += 1;
            }
            if (i - start) % 2 == 1 && b.get(i) != Some(&b'"') {
                return true;
            }
        } else {
            i += 1;
        }
    }
    false
}

/// `Util#to_string_literal`
fn to_string_literal(s: &str) -> String {
    let escaped = inspect_body(s).replace("\\\"", "\"");
    if double_quotes_required(&escaped) {
        format!("\"{}\"", inspect_body(s))
    } else {
        format!("'{}'", s.replace('\\', "\\\\").replace("\\\"", "\""))
    }
}

fn has_regopt(node: &Node<'_>) -> bool {
    let Some(re) = node.as_regular_expression_node() else { return false };
    re.is_ignore_case()
        || re.is_extended()
        || re.is_multi_line()
        || re.is_once()
        || re.is_euc_jp()
        || re.is_ascii_8bit()
        || re.is_windows_31j()
        || re.is_utf_8()
}

impl Rule for DeletePrefix {
    const META: RuleMeta = RuleMeta {
        name: "Performance/DeletePrefix",
        department: Department::Performance,
        summary: "Use `delete_prefix` instead of `gsub`.",
        explanation: "\
In Ruby 2.5, `String#delete_prefix` has been added.

This cop identifies places where `gsub(/\\Aprefix/, '')` and `sub(/\\Aprefix/, '')`
can be replaced by `delete_prefix('prefix')`.

This cop has `SafeMultiline` configuration option that `true` by default because
`^prefix` is unsafe as it will behave incompatible with `delete_prefix`
for receiver is multiline string.

This cop is unsafe because `Pathname` has `sub` but not `delete_prefix`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "SafeMultiline",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Whether `^prefix` is treated as unsafe (multiline receivers).",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            safe_multiline: options.bool("SafeMultiline"),
            enabled_for_ruby: options.target_ruby_version() >= 2.5,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled_for_ruby {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let good_method = match name.as_slice() {
            b"gsub" | b"sub" => "delete_prefix",
            b"gsub!" | b"sub!" => "delete_prefix!",
            _ => return,
        };
        let Some(receiver) = call.receiver() else { return };
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut it = arguments.arguments().iter();
        let (Some(first), Some(second), None) = (it.next(), it.next(), it.next()) else {
            return;
        };
        let Some(regexp) = first.as_regular_expression_node() else { return };
        if has_regopt(&first) {
            return;
        }
        let Some(replacement) = second.as_string_node() else { return };
        if !replacement.unescaped().is_empty() {
            return;
        }
        let regexp_str = String::from_utf8_lossy(ctx.text(regexp.content_loc().span())).into_owned();
        if !literal_at_start(&regexp_str, self.safe_multiline) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let Some(dot) = call.call_operator_loc() else { return };
        let bad_method = String::from_utf8_lossy(name.as_slice()).into_owned();
        let message = format!("Use `{good_method}` instead of `{bad_method}`.");

        let string_literal =
            to_string_literal(&interpret_string_escapes(drop_start_metacharacter(&regexp_str)));
        let mut new_code = ctx.text(receiver.span()).to_vec();
        new_code.extend_from_slice(ctx.text(dot.span()));
        new_code.extend_from_slice(format!("{good_method}({string_literal})").as_bytes());
        let end = call.closing_loc().map_or(second.span().end, |l| l.span().end);
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(Span::new(node.span().start, end), new_code)],
        };
        ctx.report_with_fix(&Self::META, selector.span(), message, fix);
    }
}
