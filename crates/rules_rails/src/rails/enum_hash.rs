//! `Rails/EnumHash`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/enum_hash.rb`.

use std::fmt::Write as _;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `TargetRailsVersion::DEFAULT_RAILS_VERSION`, used when the configuration
/// states none.
const DEFAULT_RAILS_VERSION: f64 = 5.0;

/// Looks for enums written with array syntax.
#[derive(Debug, Clone)]
pub struct EnumHash {
    /// `target_rails_version >= 7.0`.
    positional_supported: bool,
}

impl Rule for EnumHash {
    const META: RuleMeta = RuleMeta {
        name: "Rails/EnumHash",
        department: Department::Rails,
        summary: "Prefer hash syntax over array syntax when defining enums.",
        explanation: "Looks for enums written with array syntax.\n\nWhen using array syntax, \
                      adding an element in a position other than the last causes all previous \
                      definitions to shift. Explicitly specifying the value for each key \
                      prevents this from happening.\n\n```ruby\n# bad\nenum :status, [:active, \
                      :archived]\n\n# good\nenum :status, { active: 0, archived: 1 }\n\n# bad\n\
                      enum status: [:active, :archived]\n\n# good\nenum status: { active: 0, \
                      archived: 1 }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { positional_supported: target_rails_version(options) >= 7.0 })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"enum"
            || call.receiver().is_some()
            || call.is_safe_navigation()
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
        // A block-pass is a trailing argument in whitequark.
        let has_block_pass = call.block().is_some_and(|b| b.as_block_argument_node().is_some());

        // (send nil? :enum $_ ${array} ...)
        if self.positional_supported {
            if let Some(array) = arguments.get(1).filter(|arg| arg.as_array_node().is_some()) {
                register(ctx, &arguments[0], array);
            }
        }

        // (send nil? :enum (hash $...))
        if let ([only], false) = (arguments.as_slice(), has_block_pass) {
            let elements: Vec<Node<'_>> = if let Some(hash) = only.as_keyword_hash_node() {
                hash.elements().iter().collect()
            } else if let Some(hash) = only.as_hash_node() {
                hash.elements().iter().collect()
            } else {
                return;
            };
            for pair in &elements {
                let Some(assoc) = pair.as_assoc_node() else { continue };
                let value = assoc.value();
                if value.as_array_node().is_some() {
                    register(ctx, &assoc.key(), &value);
                }
            }
        }
    }
}

fn register(ctx: &mut Context<'_>, key: &Node<'_>, array: &Node<'_>) {
    let message = format!(
        "Enum defined as an array found in `{}` enum declaration. Use hash syntax instead.",
        enum_name(ctx, key)
    );
    let span: Span = array.span();
    let replacement = build_hash(ctx, array);
    ctx.report_with_fix(
        &EnumHash::META,
        span,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, replacement.into_bytes())],
        },
    );
}

fn enum_name(ctx: &Context<'_>, key: &Node<'_>) -> String {
    if let Some(symbol) = key.as_symbol_node() {
        String::from_utf8_lossy(symbol.unescaped()).into_owned()
    } else if let Some(string) = key.as_string_node() {
        String::from_utf8_lossy(string.unescaped()).into_owned()
    } else {
        String::from_utf8_lossy(ctx.text(key.span())).into_owned()
    }
}

fn build_hash(ctx: &Context<'_>, array: &Node<'_>) -> String {
    let Some(array) = array.as_array_node() else { return String::new() };
    let entries: Vec<String> = array
        .elements()
        .iter()
        .enumerate()
        .map(|(index, element)| format!("{} => {index}", element_source(ctx, &element)))
        .collect();
    format!("{{{}}}", entries.join(", "))
}

fn element_source(ctx: &Context<'_>, element: &Node<'_>) -> String {
    if let Some(string) = element.as_string_node() {
        ruby_dump(&String::from_utf8_lossy(string.unescaped()))
    } else if let Some(symbol) = element.as_symbol_node() {
        symbol_inspect(&String::from_utf8_lossy(symbol.unescaped()))
    } else {
        String::from_utf8_lossy(ctx.text(element.span())).into_owned()
    }
}

/// Ruby's `String#dump` for a UTF-8 string.
fn ruby_dump(text: &str) -> String {
    quote(text, true)
}

/// Ruby's `Symbol#inspect`.
fn symbol_inspect(name: &str) -> String {
    if is_simple_symbol(name) {
        format!(":{name}")
    } else {
        format!(":{}", quote(name, false))
    }
}

/// Double-quotes `text` the way `String#dump` (`escape_unicode`) or
/// `String#inspect` do.
fn quote(text: &str, escape_unicode: bool) -> String {
    let mut out = String::from("\"");
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{c}' => out.push_str("\\f"),
            '\u{b}' => out.push_str("\\v"),
            '\u{8}' => out.push_str("\\b"),
            '\u{7}' => out.push_str("\\a"),
            '\u{1b}' => out.push_str("\\e"),
            '#' if matches!(chars.peek(), Some('{' | '$' | '@')) => out.push_str("\\#"),
            ' '..='~' => out.push(c),
            c if c.is_ascii() => {
                let _ = write!(out, "\\x{:02X}", u32::from(c));
            }
            c if !escape_unicode => out.push(c),
            c if u32::from(c) > 0xFFFF => {
                let _ = write!(out, "\\u{{{:X}}}", u32::from(c));
            }
            c => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
        }
    }
    out.push('"');
    out
}

fn is_simple_symbol(name: &str) -> bool {
    const OPERATORS: &[&str] = &[
        "+", "-", "*", "/", "%", "**", "==", "!=", "<=>", "<", "<=", ">", ">=", "===", "=~", "!~",
        "!", "~", "+@", "-@", "[]", "[]=", "<<", ">>", "&", "|", "^", "`",
    ];
    if OPERATORS.contains(&name) {
        return true;
    }
    let ident_char = |c: char| c == '_' || c.is_alphanumeric() || !c.is_ascii();
    let identifier = |text: &str| {
        let mut chars = text.chars();
        chars.next().is_some_and(|c| c == '_' || c.is_alphabetic() || !c.is_ascii())
            && chars.all(ident_char)
    };
    if let Some(rest) = name.strip_prefix("@@").or_else(|| name.strip_prefix('@')) {
        return identifier(rest);
    }
    if let Some(rest) = name.strip_prefix('$') {
        return identifier(rest);
    }
    let body = name
        .strip_suffix('?')
        .or_else(|| name.strip_suffix('!'))
        .or_else(|| name.strip_suffix('='))
        .unwrap_or(name);
    identifier(body)
}

/// `Config#target_rails_version`: `AllCops/TargetRailsVersion` when set.
fn target_rails_version(options: &RuleOptions) -> f64 {
    match options.peer("AllCops", "TargetRailsVersion") {
        Some(OptionValue::Str(text)) => text.trim().parse().unwrap_or(DEFAULT_RAILS_VERSION),
        Some(value) => value.as_float().unwrap_or(DEFAULT_RAILS_VERSION),
        None => DEFAULT_RAILS_VERSION,
    }
}
