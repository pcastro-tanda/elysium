//! `Performance/StringIdentifierArgument`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/string_identifier_argument.rb`.

use std::fmt::Write as _;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const COMMAND_METHODS: &[&[u8]] = &[
    b"alias_method",
    b"attr_accessor",
    b"attr_reader",
    b"attr_writer",
    b"autoload",
    b"autoload?",
    b"private",
    b"private_constant",
    b"protected",
    b"public",
    b"public_constant",
    b"module_function",
];

const MULTIPLE_ARGUMENTS_METHODS: &[&[u8]] = &[
    b"attr_accessor",
    b"attr_reader",
    b"attr_writer",
    b"private",
    b"private_constant",
    b"protected",
    b"public",
    b"public_constant",
    b"module_function",
];

const RESTRICT_ON_SEND: &[&[u8]] = &[
    b"class_variable_defined?",
    b"const_set",
    b"define_method",
    b"instance_method",
    b"method_defined?",
    b"private_class_method?",
    b"private_method_defined?",
    b"protected_method_defined?",
    b"public_class_method",
    b"public_instance_method",
    b"public_method_defined?",
    b"remove_class_variable",
    b"remove_method",
    b"undef_method",
    b"class_variable_get",
    b"class_variable_set",
    b"deprecate_constant",
    b"remove_const",
    b"ruby2_keywords",
    b"define_singleton_method",
    b"instance_variable_defined?",
    b"instance_variable_get",
    b"instance_variable_set",
    b"method",
    b"public_method",
    b"public_send",
    b"remove_instance_variable",
    b"respond_to?",
    b"send",
    b"singleton_method",
    b"__send__",
];

/// Use symbol identifier argument instead of string identifier argument.
#[derive(Debug, Clone)]
pub struct StringIdentifierArgument;

impl Rule for StringIdentifierArgument {
    const META: RuleMeta = RuleMeta {
        name: "Performance/StringIdentifierArgument",
        department: Department::Performance,
        summary: "Use symbol identifier argument instead of string identifier argument.",
        explanation: "Identifies places where string identifier argument can be replaced by \
                      symbol identifier argument. It prevents the redundancy of the internal \
                      string-to-symbol conversion.\n\n```ruby\n# bad\nsend('do_something')\n\
                      attr_accessor 'do_something'\ninstance_variable_get('@ivar')\n\n# good\n\
                      send(:do_something)\nattr_accessor :do_something\n\
                      instance_variable_get(:@ivar)\n```",
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
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        let is_command = COMMAND_METHODS.contains(&name);
        if !is_command && !RESTRICT_ON_SEND.contains(&name) {
            return;
        }
        if is_command && call.receiver().is_some() {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let all: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let candidates: &[Node<'_>] = if name == b"alias_method" {
            &all[..all.len().min(2)]
        } else if MULTIPLE_ARGUMENTS_METHODS.contains(&name) {
            &all
        } else {
            &all[..1]
        };
        for argument in candidates {
            let Some(string) = argument.as_string_node() else { continue };
            let value = string.unescaped();
            if value.contains(&b' ') || value.windows(2).any(|w| w == b"::") {
                continue;
            }
            let span = argument.span();
            let replacement = symbol_inspect(value);
            let message = format!(
                "Use `{replacement}` instead of `{}`.",
                String::from_utf8_lossy(ctx.text(span))
            );
            ctx.report_with_fix(
                &Self::META,
                span,
                message,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, replacement.into_bytes())],
                },
            );
        }
    }
}

/// `String#to_sym.inspect`.
fn symbol_inspect(value: &[u8]) -> String {
    match std::str::from_utf8(value) {
        Ok(text) if is_simple_symbol(text) => format!(":{text}"),
        Ok(text) => format!(":\"{}\"", inspect_body(text)),
        Err(_) => {
            let mut out = String::from(":\"");
            for b in value {
                let _ = write!(out, "\\x{b:02X}");
            }
            out.push('"');
            out
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic() || !c.is_ascii()
}

fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric() || !c.is_ascii()
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(is_ident_start) && chars.all(is_ident_char)
}

/// Whether `Symbol#inspect` prints `text` without quotes.
fn is_simple_symbol(text: &str) -> bool {
    const OPERATORS: &[&str] = &[
        "[]", "[]=", "+", "-", "*", "/", "%", "**", "!", "!=", "!~", "=~", "==", "===", "<=>", "<",
        "<=", ">", ">=", "<<", ">>", "~", "+@", "-@", "&", "|", "^", "`",
    ];
    if OPERATORS.contains(&text) {
        return true;
    }
    if let Some(rest) = text.strip_prefix("@@").or_else(|| text.strip_prefix('@')) {
        return is_identifier(rest);
    }
    if let Some(rest) = text.strip_prefix('$') {
        if is_identifier(rest) {
            return true;
        }
        if rest.len() == 1 && "~*$?!@/\\;,.=:<>\"&`'+".contains(rest) {
            return true;
        }
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            return true;
        }
        return rest.strip_prefix('-').is_some_and(|r| r.chars().count() == 1 && r.chars().all(is_ident_char));
    }
    let base = text
        .strip_suffix(['?', '!', '='])
        .filter(|b| !b.is_empty())
        .unwrap_or(text);
    is_identifier(base)
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
