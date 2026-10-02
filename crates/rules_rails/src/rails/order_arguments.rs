//! `Rails/OrderArguments`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/order_arguments.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::sync::OnceLock;

/// Prefer symbol arguments over strings in `order` method.
#[derive(Debug, Clone)]
pub struct OrderArguments;

/// `ORDER_EXPRESSION_REGEX`: `/\A(\w+) ?(asc|desc)?\z/i` (Ruby's `\w` is
/// ASCII-only).
fn order_expression() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\A([A-Za-z0-9_]+) ?(asc|desc)?\z").expect("static regex is valid")
    })
}

impl Rule for OrderArguments {
    const META: RuleMeta = RuleMeta {
        name: "Rails/OrderArguments",
        department: Department::Rails,
        summary: "Prefer symbol arguments over strings in `order` method.",
        explanation: "Prefer symbol arguments over strings in `order` method.\n\nCop is unsafe \
                      because the receiver might not be an Active Record query.\n\n```ruby\n\
                      # bad\nUser.order('name')\nUser.order('name DESC')\n\n# good\n\
                      User.order(:name)\nUser.order(name: :desc)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
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
        if call.name().as_slice() != b"order" {
            return;
        }
        // `(call _ :order (str $_value)+)`: every argument a plain string
        // (and no `&block`, which whitequark counts as an argument).
        if call.block().is_some_and(|block| block.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let mut expressions = Vec::with_capacity(arguments.len());
        for argument in &arguments {
            let Some(string) = argument.as_string_node() else { return };
            expressions.push(String::from_utf8_lossy(string.unescaped()).into_owned());
        }
        let (Some(first), Some(last)) = (arguments.first(), arguments.last()) else { return };
        let Some(preferred) = replacement(&expressions) else { return };

        let range = Span::new(first.span().start, last.span().end);
        ctx.report_with_fix(
            &Self::META,
            range,
            format!("Prefer `{preferred}` instead."),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, preferred.into_bytes())],
            },
        );
    }
}

/// Ruby's `String#split(',')`: trailing empty fields are dropped.
fn split_commas(expression: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = expression.split(',').collect();
    while parts.last().is_some_and(|part| part.is_empty()) {
        parts.pop();
    }
    parts
}

/// Ruby's `String#strip` (ASCII whitespace and NUL).
fn strip(text: &str) -> &str {
    text.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r' | '\0'))
}

/// `replacement`: the preferred arguments, or `None` when any expression is
/// not a plain `column [asc|desc]` or names a positional column.
fn replacement(order_expressions: &[String]) -> Option<String> {
    let mut order_arguments: Vec<(String, bool)> = Vec::new();
    for expression in order_expressions {
        for argument in split_commas(expression) {
            // `extract_column_and_direction`: (downcased column, is asc).
            let captures = order_expression().captures(strip(argument))?;
            let column = captures[1].to_ascii_lowercase();
            let ascending =
                captures.get(2).is_none_or(|direction| direction.as_str().eq_ignore_ascii_case("asc"));
            order_arguments.push((column, ascending));
        }
    }
    // `positional_column?`.
    if order_arguments.iter().any(|(column, _)| column.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    // `convert_to_preferred_arguments`.
    let mut use_hash = false;
    let converted: Vec<String> = order_arguments
        .iter()
        .map(|(column, ascending)| {
            if *ascending && !use_hash {
                format!(":{column}")
            } else {
                use_hash = true;
                format!("{column}: :{}", if *ascending { "asc" } else { "desc" })
            }
        })
        .collect();
    Some(converted.join(", "))
}
