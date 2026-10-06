//! `Rails/WhereRange`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/where_range.rb`.

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util;

/// `minimum_target_rails_version 6.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 6.0;

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static pattern")
}

// Ruby's `\w`, `\s` are ASCII-only, hence `(?-u)`.
/// `column >= ?`
static GTEQ_ANONYMOUS_RE: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?-u)\A\s*([\w.]+)\s+>=\s+\?\s*\z"));
/// `column <[=] ?`
static LTEQ_ANONYMOUS_RE: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?-u)\A\s*([\w.]+)\s+(<=?)\s+\?\s*\z"));
/// `column >= ? AND column <[=] ?`; the `\1` backreference is checked by
/// hand (case-insensitively, like the `/i` it sits under).
static RANGE_ANONYMOUS_RE: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?-u)\A\s*([\w.]+)\s+>=\s+\?\s+(?i:AND)\s+([\w.]+)\s+(<=?)\s+\?\s*\z")
});
/// `column >= :value`
static GTEQ_NAMED_RE: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?-u)\A\s*([\w.]+)\s+>=\s+:(\w+)\s*\z"));
/// `column <[=] :value`
static LTEQ_NAMED_RE: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?-u)\A\s*([\w.]+)\s+(<=?)\s+:(\w+)\s*\z"));
/// `column >= :value1 AND column <[=] :value2`
static RANGE_NAMED_RE: LazyLock<Regex> = LazyLock::new(|| {
    regex(r"(?-u)\A\s*([\w.]+)\s+>=\s+:(\w+)\s+(?i:AND)\s+([\w.]+)\s+(<=?)\s+:(\w+)\s*\z")
});

/// Use ranges in `where` instead of manually constructing SQL.
#[derive(Debug, Clone)]
pub struct WhereRange {
    /// `minimum_target_ruby_version 2.6` and
    /// `minimum_target_rails_version 6.0` both hold.
    supported: bool,
    /// `target_ruby_version >= 2.7`: beginless ranges exist.
    beginless: bool,
}

impl Rule for WhereRange {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereRange",
        department: Department::Rails,
        summary: "Use ranges in `where` instead of manually constructing SQL.",
        explanation: "Identifies places where manually constructed SQL in `where` can be \
                      replaced with ranges.\n\nThis cop's autocorrection is unsafe because it \
                      can change the query by explicitly attaching the column to the wrong \
                      table. For example, `Booking.joins(:events).where('end_at < ?', \
                      Time.current)` will correctly implicitly attach the `end_at` column to \
                      the `events` table. But when autocorrected to \
                      `Booking.joins(:events).where(end_at: ...Time.current)`, it will now be \
                      incorrectly explicitly attached to the `bookings` table.\n\n```ruby\n# \
                      bad\nUser.where('age >= ?', 18)\nUser.where.not('age >= ?', \
                      18)\nUser.where('age < ?', 18)\nUser.where('age >= ? AND age < ?', 18, \
                      21)\nUser.where('age >= :start', start: 18)\nUser.where('users.age >= \
                      ?', 18)\n\n# good\nUser.where(age: 18..)\nUser.where.not(age: \
                      18..)\nUser.where(age: ...18)\nUser.where(age: 18...21)\nUser.where(users: \
                      { age: 18.. })\n\n# good\n# There are no beginless ranges in ruby.\n\
                      User.where('age > ?', 18)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let ruby = options.target_ruby_version();
        Ok(Self {
            supported: ruby >= 2.6
                && options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION,
            beginless: ruby >= 2.7,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let name = name.as_slice();
        if name != b"where" && name != b"not" {
            return;
        }
        if name == b"not" {
            // `where_not?`: the receiver is a `where` `send`.
            let is_where = call
                .receiver()
                .and_then(|r| r.as_call_node())
                .is_some_and(|r| !r.is_safe_navigation() && r.name().as_slice() == b"where");
            if !is_where {
                return;
            }
        }
        let Some((template, values)) = where_range_call(&call) else { return };
        let Some((column, value)) = self.extract_column_and_value(&template, &values, ctx) else {
            return;
        };
        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, node.span().end);
        let method = String::from_utf8_lossy(name);
        let good_method = build_good_method(&method, &column, &value);
        let message = format!("Use `{good_method}` instead of manually constructing SQL.");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, good_method.into_bytes())],
            },
        );
    }
}

/// `where_range_call?`: `(call _ {:where :not} (array $str $_ +))` or
/// `(call _ {:where :not} $str $_ +)`; yields the template and the rest.
fn where_range_call<'pr>(
    call: &ruby_ast::node::CallNode<'pr>,
) -> Option<(ruby_ast::node::StringNode<'pr>, Vec<Node<'pr>>)> {
    let args = util::parser_args(call);
    let list: Vec<Node<'pr>> = match args.as_slice() {
        [single] => single.as_array_node()?.elements().iter().collect(),
        _ => args,
    };
    let mut list = list.into_iter();
    let template = list.next()?.as_string_node()?;
    let values: Vec<Node<'pr>> = list.collect();
    if values.is_empty() {
        return None;
    }
    Some((template, values))
}

impl WhereRange {
    /// `extract_column_and_value`: the column and the range source.
    fn extract_column_and_value(
        &self,
        template: &ruby_ast::node::StringNode<'_>,
        values: &[Node<'_>],
        ctx: &Context<'_>,
    ) -> Option<(String, String)> {
        let template = std::str::from_utf8(template.unescaped()).ok()?;
        let mut lhs: Option<&Node<'_>> = None;
        let mut rhs: Option<&Node<'_>> = None;
        let mut operator: Option<&str> = None;
        let column: &str;
        let pair_lhs;
        let pair_rhs;
        if let Some(c) = GTEQ_ANONYMOUS_RE.captures(template) {
            column = c.get(1)?.as_str();
            lhs = values.first();
            operator = Some("..");
        } else if let Some(c) = LTEQ_ANONYMOUS_RE.captures(template) {
            column = c.get(1)?.as_str();
            if self.beginless {
                operator = Some(range_operator(c.get(2)?.as_str()));
                rhs = values.first();
            }
        } else if let Some(c) =
            RANGE_ANONYMOUS_RE.captures(template).filter(|c| c[1].eq_ignore_ascii_case(&c[2]))
        {
            column = c.get(1)?.as_str();
            if values.len() >= 2 {
                lhs = values.first();
                operator = Some(range_operator(c.get(3)?.as_str()));
                rhs = values.get(1);
            }
        } else if let Some(c) = GTEQ_NAMED_RE.captures(template) {
            column = c.get(1)?.as_str();
            if let Some(hash) = values.first().and_then(hash_pairs) {
                pair_lhs = find_pair(&hash, c.get(2)?.as_str())?;
                lhs = Some(&pair_lhs);
                operator = Some("..");
            }
        } else if let Some(c) = LTEQ_NAMED_RE.captures(template) {
            column = c.get(1)?.as_str();
            if let Some(hash) = values.first().and_then(hash_pairs) {
                // Upstream looks the pair up by `last_match(2)`, which is the
                // comparison operator rather than the value name.
                let found = find_pair(&hash, c.get(2)?.as_str());
                if let (Some(pair), true) = (found, self.beginless) {
                    operator = Some(range_operator(c.get(2)?.as_str()));
                    pair_rhs = pair;
                    rhs = Some(&pair_rhs);
                }
            }
        } else {
            let c =
                RANGE_NAMED_RE.captures(template).filter(|c| c[1].eq_ignore_ascii_case(&c[3]))?;
            column = c.get(1)?.as_str();
            if let Some(hash) = values.first().and_then(hash_pairs) {
                let first = find_pair(&hash, c.get(2)?.as_str());
                let second = find_pair(&hash, c.get(5)?.as_str());
                if let (Some(a), Some(b)) = (first, second) {
                    pair_lhs = a;
                    pair_rhs = b;
                    lhs = Some(&pair_lhs);
                    operator = Some(range_operator(c.get(4)?.as_str()));
                    rhs = Some(&pair_rhs);
                }
            }
        }
        let operator = operator?;
        if column.matches('.').count() > 1 {
            return None;
        }
        let source = |node: Option<&Node<'_>>| {
            node.map(|node| {
                let mut text = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
                // Prism's value of an omitted-value pair (`key:`) spans the
                // trailing colon; parser's send/lvar node does not.
                let omitted_value = node.as_call_node().is_some()
                    || node.as_local_variable_read_node().is_some()
                    || node.as_constant_read_node().is_some();
                if omitted_value && text.ends_with(':') {
                    text.pop();
                }
                if parentheses_needed(node) {
                    format!("({text})")
                } else {
                    text
                }
            })
            .unwrap_or_default()
        };
        Some((column.to_owned(), format!("{}{operator}{}", source(lhs), source(rhs))))
    }
}

/// The pairs of a hash literal (braced or bare keywords).
fn hash_pairs<'pr>(node: &Node<'pr>) -> Option<Vec<ruby_ast::node::AssocNode<'pr>>> {
    let elements = match node.as_hash_node() {
        Some(hash) => hash.elements(),
        None => node.as_keyword_hash_node()?.elements(),
    };
    Some(elements.iter().filter_map(|e| e.as_assoc_node()).collect())
}

/// `find_pair`: the value of the first pair whose key's value is `name`.
fn find_pair<'pr>(pairs: &[ruby_ast::node::AssocNode<'pr>], name: &str) -> Option<Node<'pr>> {
    pairs
        .iter()
        .find(|pair| {
            let key = pair.key();
            let value = match (key.as_symbol_node(), key.as_string_node()) {
                (Some(symbol), _) => symbol.unescaped().to_vec(),
                (None, Some(string)) => string.unescaped().to_vec(),
                (None, None) => return false,
            };
            value == name.as_bytes()
        })
        .map(|pair| {
            let value = pair.value();
            value.as_implicit_node().map_or(value, |implicit| implicit.value())
        })
}

fn range_operator(comparison: &str) -> &'static str {
    if comparison == "<" {
        "..."
    } else {
        ".."
    }
}

fn build_good_method(method: &str, column: &str, value: &str) -> String {
    if column.contains('.') {
        // Ruby's `split('.')` drops trailing empty fields.
        let mut parts: Vec<&str> = column.split('.').collect();
        while parts.last() == Some(&"") {
            parts.pop();
        }
        let table = parts.first().copied().unwrap_or("");
        let column = parts.get(1).copied().unwrap_or("");
        format!("{method}({table}: {{ {column}: {value} }})")
    } else {
        format!("{method}({column}: {value})")
    }
}

/// `!parentheses_not_needed?(node)`.
fn parentheses_needed(node: &Node<'_>) -> bool {
    !(node.as_local_variable_read_node().is_some()
        || node.as_instance_variable_read_node().is_some()
        || node.as_class_variable_read_node().is_some()
        || node.as_global_variable_read_node().is_some()
        || is_literal(node)
        || node.as_numbered_reference_read_node().is_some()
        || node.as_back_reference_read_node().is_some()
        || node.as_constant_read_node().is_some()
        || node.as_constant_path_node().is_some()
        || node.as_parentheses_node().is_some()
        || is_parenthesized_call(node))
}

/// `Node#literal?`.
fn is_literal(node: &Node<'_>) -> bool {
    node.as_string_node().is_some()
        || node.as_interpolated_string_node().is_some()
        || node.as_x_string_node().is_some()
        || node.as_interpolated_x_string_node().is_some()
        || node.as_integer_node().is_some()
        || node.as_float_node().is_some()
        || node.as_symbol_node().is_some()
        || node.as_interpolated_symbol_node().is_some()
        || node.as_array_node().is_some()
        || node.as_hash_node().is_some()
        || node.as_keyword_hash_node().is_some()
        || node.as_regular_expression_node().is_some()
        || node.as_interpolated_regular_expression_node().is_some()
        || node.as_true_node().is_some()
        || node.as_false_node().is_some()
        || node.as_nil_node().is_some()
        || node.as_range_node().is_some()
        || node.as_imaginary_node().is_some()
        || node.as_rational_node().is_some()
        || node.as_source_file_node().is_some()
        || node.as_source_line_node().is_some()
}

/// `node.call_type? && (node.arguments.empty? || node.parenthesized_call?)`;
/// a call with a literal block is a `block` node there.
fn is_parenthesized_call(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.block().is_some_and(|block| block.as_block_node().is_some()) {
        return false;
    }
    util::parser_args(&call).is_empty() || call.opening_loc().is_some_and(|o| o.as_slice() == b"(")
}
