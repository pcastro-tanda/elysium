//! `Rails/EnumSyntax`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/enum_syntax.rb`.

use std::fmt::Write as _;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG_PREFIX: &str = "Enum defined with keyword arguments in `";
const MSG_SUFFIX: &str = "` enum declaration. Use positional arguments instead.";
const MSG_OPTIONS_PREFIX: &str = "Enum defined with deprecated options in `";
const MSG_OPTIONS_SUFFIX: &str = "` enum declaration. Remove the `_` prefix.";

/// `OPTION_NAMES`.
const OPTION_NAMES: [&str; 5] = ["prefix", "suffix", "scopes", "default", "instance_methods"];
/// `UNDERSCORED_OPTION_NAMES`.
const UNDERSCORED_OPTION_NAMES: [&str; 5] =
    ["_prefix", "_suffix", "_scopes", "_default", "_instance_methods"];

/// Use positional arguments over keyword arguments when defining enums.
#[derive(Debug, Clone)]
pub struct EnumSyntax {
    /// `minimum_target_ruby_version 3.0` and `minimum_target_rails_version 7.0`.
    enabled: bool,
}

impl Rule for EnumSyntax {
    const META: RuleMeta = RuleMeta {
        name: "Rails/EnumSyntax",
        department: Department::Rails,
        summary: "Use positional arguments over keyword arguments when defining enums.",
        explanation: "Looks for enums written with keyword arguments syntax.\n\nDefining enums \
                      with keyword arguments syntax is deprecated and will be removed in Rails \
                      8.0. Positional arguments should be used instead:\n\n```ruby\n# bad\nenum \
                      status: { active: 0, archived: 1 }, _prefix: true\n\n# good\nenum :status, \
                      { active: 0, archived: 1 }, prefix: true\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            enabled: options.target_ruby_version() >= 3.0 && options.target_rails_version() >= 7.0,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.enabled {
            return;
        }
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
        let argc = arguments.len() + usize::from(has_block_pass);

        // check_and_correct_keyword_args: (send nil? :enum (hash $...))
        if argc == 1 {
            if let Some(pairs) = hash_elements(&arguments[0]) {
                let call_span = call_span_excluding_block(&call);
                check_keyword_args(ctx, call_span, &pairs);
            }
        }

        // check_enum_options: (send nil? :enum $_ ${array hash} $_)
        if argc == 3 {
            let second = &arguments[1];
            if second.as_array_node().is_none() && hash_elements(second).is_none() {
                return;
            }
            let Some(options) = hash_elements(&arguments[2]) else { return };
            let name = enum_name_value(ctx, &arguments[0]);
            for option in &options {
                let Some(assoc) = option.as_assoc_node() else { continue };
                if !option_key(ctx, &assoc) {
                    continue;
                }
                let span = key_span(ctx, &assoc.key());
                let source = ctx.text(span).to_vec();
                let replacement = source.strip_prefix(b"_").unwrap_or(&source).to_vec();
                ctx.report_with_fix(
                    &Self::META,
                    span,
                    format!("{MSG_OPTIONS_PREFIX}{name}{MSG_OPTIONS_SUFFIX}"),
                    Fix {
                        applicability: Applicability::Safe,
                        edits: vec![Edit::replace(span, replacement)],
                    },
                );
            }
        }
    }
}

/// The elements of a hash literal (braced or bare keywords).
fn hash_elements<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    if let Some(hash) = node.as_keyword_hash_node() {
        Some(hash.elements().iter().collect())
    } else {
        node.as_hash_node().map(|hash| hash.elements().iter().collect())
    }
}

fn check_keyword_args(ctx: &mut Context<'_>, call_span: Span, pairs: &[Node<'_>]) {
    let assocs: Vec<_> = pairs.iter().filter_map(ruby_ast::Node::as_assoc_node).collect();
    // `pairs[1..]` in upstream (kwsplats would raise there; skipped here).
    for pair in pairs {
        let Some(assoc) = pair.as_assoc_node() else { continue };
        if option_key(ctx, &assoc) {
            continue;
        }
        let key = assoc.key();
        let value = assoc.value();
        let value_span = value.span();
        let message = format!("{MSG_PREFIX}{}{MSG_SUFFIX}", enum_name_value(ctx, &key));
        if multiple_enum_definitions(ctx, &assocs) {
            ctx.report(&EnumSyntax::META, value_span, message);
            continue;
        }
        // `pairs[1..]`: everything but the first pair.
        let rest: Vec<_> = pairs[1..].iter().filter_map(ruby_ast::Node::as_assoc_node).collect();
        let preferred = format!(
            "enum {}, {}{}",
            enum_name(ctx, &key),
            String::from_utf8_lossy(ctx.text(value_span)),
            correct_options(ctx, &rest),
        );
        ctx.report_with_fix(
            &EnumSyntax::META,
            value_span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(call_span, preferred.into_bytes())],
            },
        );
    }
}

fn multiple_enum_definitions(ctx: &Context<'_>, assocs: &[ruby_ast::node::AssocNode<'_>]) -> bool {
    assocs
        .iter()
        .filter(|assoc| {
            let source = ctx.text(key_span(ctx, &assoc.key()));
            let key = source.strip_prefix(b"_").unwrap_or(source);
            !OPTION_NAMES.iter().any(|name| name.as_bytes() == key)
        })
        .count()
        >= 2
}

fn correct_options(ctx: &Context<'_>, options: &[ruby_ast::node::AssocNode<'_>]) -> String {
    let corrected: Vec<String> = options
        .iter()
        .map(|pair| {
            let source = ctx.text(key_span(ctx, &pair.key()));
            let name = source.strip_prefix(b"_").unwrap_or(source);
            format!(
                "{}: {}",
                String::from_utf8_lossy(name),
                String::from_utf8_lossy(ctx.text(pair.value().span()))
            )
        })
        .collect();
    if corrected.is_empty() {
        String::new()
    } else {
        format!(", {}", corrected.join(", "))
    }
}

/// `pair.key.source`: a label key (`status:`) excludes the colon in
/// whitequark, but Prism's symbol node includes it.
fn key_span(ctx: &Context<'_>, key: &Node<'_>) -> Span {
    let span = key.span();
    if key.as_symbol_node().is_some() && ctx.text(span).ends_with(b":") {
        Span::new(span.start, span.end - 1)
    } else {
        span
    }
}

fn option_key(ctx: &Context<'_>, assoc: &ruby_ast::node::AssocNode<'_>) -> bool {
    let source = ctx.text(key_span(ctx, &assoc.key()));
    UNDERSCORED_OPTION_NAMES.iter().any(|name| name.as_bytes() == source)
}

fn enum_name_value(ctx: &Context<'_>, key: &Node<'_>) -> String {
    if let Some(symbol) = key.as_symbol_node() {
        String::from_utf8_lossy(symbol.unescaped()).into_owned()
    } else if let Some(string) = key.as_string_node() {
        String::from_utf8_lossy(string.unescaped()).into_owned()
    } else {
        String::from_utf8_lossy(ctx.text(key.span())).into_owned()
    }
}

fn enum_name(ctx: &Context<'_>, key: &Node<'_>) -> String {
    if let Some(symbol) = key.as_symbol_node() {
        symbol_inspect(&String::from_utf8_lossy(symbol.unescaped()))
    } else if let Some(string) = key.as_string_node() {
        ruby_dump(&String::from_utf8_lossy(string.unescaped()))
    } else {
        String::from_utf8_lossy(ctx.text(key.span())).into_owned()
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
