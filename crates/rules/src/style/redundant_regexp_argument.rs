//! `Style/RedundantRegexpArgument`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_regexp_argument.rb` plus the
//! `RuboCop::Cop::Util::LITERAL_REGEX` constant and `StringLiteralsHelp`
//! mixin it relies on.
//!
//! Upstream's `(regopt)` child node (the trailing `i`/`m`/`x`/`o`/encoding
//! flag letters of a regexp literal) has no direct Prism equivalent; Prism
//! instead exposes only `is_ignore_case`/`is_extended`/`is_multi_line`/
//! `is_once` booleans, which are false unless the matching flag letter was
//! actually typed (the same four flags already relied on by
//! `style/redundant_regexp_character_class.rs` and
//! `style/redundant_regexp_escape.rs`), so `has_regexp_options` checks those
//! four instead of walking a `regopt` node.
//!
//! `DETERMINISTIC_REGEX`/`LITERAL_REGEX` are ported byte-for-byte as
//! [`is_deterministic_regexp_source`], scanning the regexp's own full
//! literal source (opening/closing delimiters included, since `/` is itself
//! one of `LITERAL_REGEX`'s allowed literal characters).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions,
};
use linter::{OptionValue, Severity, Stability};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const RESTRICT_ON_SEND: &[&[u8]] = &[
    b"byteindex",
    b"byterindex",
    b"gsub",
    b"gsub!",
    b"partition",
    b"rpartition",
    b"scan",
    b"split",
    b"start_with?",
    b"sub",
    b"sub!",
];

/// `RuboCop::Cop::Util::LITERAL_REGEX`'s single-character alternative: word,
/// space, and a fixed set of punctuation characters that never need
/// escaping in a regexp.
fn is_safe_literal_char(c: char) -> bool {
    c.is_alphanumeric()
        || c == '_'
        || c.is_whitespace()
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
        )
}

/// `RuboCop::Cop::Util::LITERAL_REGEX`'s escaped-character alternative:
/// `\\[^AbBdDgGhHkpPRwWXsSzZ0-9]` -- a backslash followed by anything other
/// than a "real" regexp feature letter/backreference digit.
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

/// `DETERMINISTIC_REGEX = /\A(?:#{LITERAL_REGEX})+\Z/`: every character of
/// `source` is either a safe literal char or part of a safe escaped pair.
fn is_deterministic_regexp_source(source: &str) -> bool {
    let mut chars = source.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let Some(next) = chars.next() else { return false };
            if !is_safe_escaped_char(next) {
                return false;
            }
        } else if !is_safe_literal_char(c) {
            return false;
        }
    }
    true
}

/// RuboCop's `STR_SPECIAL_CHARS`: backslash-escaped sequences that must keep
/// their backslash when carried over into a string literal.
const STR_SPECIAL_CHARS: &[&str] = &[
    "\\a", "\\c", "\\C", "\\e", "\\f", "\\M", "\\n", "\\\"", "\\'", "\\\\", "\\t", "\\b", "\\r",
    "\\u", "\\v", "\\x", "\\0", "\\1", "\\2", "\\3", "\\4", "\\5", "\\6", "\\7",
];

/// RuboCop's `replacement`: groups the regexp's content into backslash-pairs
/// and lone characters, then strips the backslash from any pair that is not
/// one of `STR_SPECIAL_CHARS` (i.e. an escape that is only meaningful inside
/// a regexp, not a string).
fn regexp_content_to_string(content: &str) -> String {
    let mut groups: Vec<String> = Vec::new();
    let mut chars = content.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) => groups.push(format!("\\{next}")),
                None => groups.push("\\".to_string()),
            }
        } else {
            groups.push(c.to_string());
        }
    }
    groups
        .into_iter()
        .map(|group| {
            if STR_SPECIAL_CHARS.contains(&group.as_str()) {
                group
            } else {
                group.replace('\\', "")
            }
        })
        .collect()
}

/// RuboCop's `preferred_argument`: picks a quoting style and escapes the
/// converted content to fit it.
fn preferred_argument(content: &str, enforce_double_quotes: bool) -> String {
    let mut new_argument = regexp_content_to_string(content);
    let quote = if new_argument.contains('"') {
        new_argument = new_argument.replace('\'', "\\'");
        new_argument = new_argument.replace("\\\"", "\"");
        '\''
    } else if new_argument.contains("\\'") {
        new_argument = escape_unescaped_single_quotes(&new_argument);
        '\''
    } else if new_argument.contains('\'') {
        new_argument = new_argument.replace('\'', "\\'");
        '\''
    } else if new_argument.contains('\\') || enforce_double_quotes {
        '"'
    } else {
        '\''
    };
    format!("{quote}{new_argument}{quote}")
}

/// RuboCop's `new_argument.gsub!(/(?<!\\)((?:\\\\)*)'/) { "#{$1}\\'" }`: add
/// a backslash before every `'` that is not already escaped (preceded by an
/// even number of backslashes).
fn escape_unescaped_single_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut backslash_run = 0usize;
    for c in s.chars() {
        if c == '\\' {
            backslash_run += 1;
            out.push(c);
        } else if c == '\'' && backslash_run % 2 == 0 {
            out.push('\\');
            out.push(c);
            backslash_run = 0;
        } else {
            out.push(c);
            backslash_run = 0;
        }
    }
    out
}

/// Whether any regexp option letter was present on the literal: RuboCop's
/// `!regexp_node.regopt.children.empty?`.
fn has_regexp_options(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::RegularExpressionNode => {
            let n = node.as_regular_expression_node().expect("kind matched");
            n.is_ignore_case() || n.is_extended() || n.is_multi_line() || n.is_once()
        }
        NodeKind::InterpolatedRegularExpressionNode => {
            let n = node.as_interpolated_regular_expression_node().expect("kind matched");
            n.is_ignore_case() || n.is_extended() || n.is_multi_line() || n.is_once()
        }
        _ => false,
    }
}

/// Identifies places where argument can be replaced from a deterministic
/// regexp to a string.
///
/// # Examples
///
/// ```ruby
/// # bad
/// 'foo'.gsub(/f/, 'x')
///
/// # good
/// 'foo'.gsub('f', 'x')
/// ```
#[derive(Debug, Clone)]
pub struct RedundantRegexpArgument {
    enforce_double_quotes: bool,
}

impl Rule for RedundantRegexpArgument {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantRegexpArgument",
        department: Department::Style,
        summary: "Identifies places where argument can be replaced from a deterministic regexp to a string.",
        explanation: "\
Identifies places where argument can be replaced from a deterministic regexp \
to a string.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Interpolated regexp literals (`InterpolatedRegularExpressionNode`) are
accepted for the options check but never produce a replacement string: their
content is not reassembled from parts (no fixture exercises a deterministic
interpolated-regexp first argument, since any `#{...}`/`{`/`}` text fails the
upstream `DETERMINISTIC_REGEX` character class anyway).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let enforce_double_quotes = matches!(
            options.peer("Style/StringLiterals", "EnforcedStyle"),
            Some(OptionValue::Str(s)) if s == "double_quotes"
        );
        Ok(Self { enforce_double_quotes })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let name = call.name();
        if !RESTRICT_ON_SEND.contains(&name.as_slice()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(regexp_node) = arguments.arguments().iter().next() else { return };
        if !matches!(
            regexp_node.kind(),
            NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode
        ) {
            return;
        }
        if has_regexp_options(&regexp_node) {
            return;
        }
        let Some(re) = regexp_node.as_regular_expression_node() else { return };
        let content = String::from_utf8_lossy(ctx.text(re.content_loc().span())).into_owned();
        if content == " " {
            return;
        }

        let span = regexp_node.location().span();
        let source = String::from_utf8_lossy(ctx.text(span)).into_owned();
        if !is_deterministic_regexp_source(&source) {
            return;
        }

        let prefer = preferred_argument(&content, self.enforce_double_quotes);
        let message = format!("Use string `{prefer}` as argument instead of regexp `{source}`.");

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, prefer.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
