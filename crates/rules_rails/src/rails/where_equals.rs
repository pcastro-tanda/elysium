//! `Rails/WhereEquals`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/where_equals.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::sync::OnceLock;

/// Pass conditions to `where` and `where.not` as a hash instead of manually
/// constructing SQL.
#[derive(Debug, Clone)]
pub struct WhereEquals;

impl Rule for WhereEquals {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereEquals",
        department: Department::Rails,
        summary: "Pass conditions to `where` and `where.not` as a hash instead of manually \
                  constructing SQL.",
        explanation: "Identifies places where manually constructed SQL in `where` and \
                      `where.not` can be replaced with `where(attribute: value)` and \
                      `where.not(attribute: value)`.\n\nThis cop's autocorrection is unsafe \
                      because is may change SQL. See: \
                      https://github.com/rubocop/rubocop-rails/issues/403\n\n```ruby\n# bad\n\
                      User.where('name = ?', 'Gabe')\nUser.where.not('name = ?', 'Gabe')\n\
                      User.where('name = :name', name: 'Gabe')\nUser.where('name IS NULL')\n\
                      User.where('name IN (?)', ['john', 'jane'])\nUser.where('name IN \
                      (:names)', names: ['john', 'jane'])\nUser.where('users.name = :name', \
                      name: 'Gabe')\n\n# good\nUser.where(name: 'Gabe')\n\
                      User.where.not(name: 'Gabe')\nUser.where(name: nil)\n\
                      User.where(name: ['john', 'jane'])\nUser.where(users: { name: 'Gabe' })\n\
                      ```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        let is_not = match name.as_slice() {
            b"where" => false,
            b"not" => true,
            _ => return,
        };
        if is_not && !is_where_not(&call) {
            return;
        }
        let Some((template, value)) = where_method_call(&call) else { return };
        let Some(selector) = call.message_loc() else { return };
        let range = Span::new(selector.span().start, call_span_excluding_block(&call).end);

        let Some((column, value)) = extract_column_and_value(ctx, &template, value.as_ref()) else {
            return;
        };
        let good_method = build_good_method(if is_not { "not" } else { "where" }, &column, &value);
        ctx.report_with_fix(
            &Self::META,
            range,
            format!("Use `{good_method}` instead of manually constructing SQL."),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(range, good_method.into_bytes())],
            },
        );
    }
}

/// `where_not?`: the receiver is a `where` send (not a safe-navigation one).
fn is_where_not(call: &CallNode<'_>) -> bool {
    call.receiver().and_then(|receiver| receiver.as_call_node()).is_some_and(|receiver| {
        receiver.name().as_slice() == b"where" && !receiver.is_safe_navigation()
    })
}

/// `where_method_call?`: a string template and at most one more value, either
/// as the arguments or wrapped in a single array argument.
fn where_method_call<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Option<Node<'pr>>)> {
    let arguments: Vec<Node<'pr>> = call.arguments()?.arguments().iter().collect();
    let elements: Vec<Node<'pr>> = match arguments.as_slice() {
        [single] if single.as_array_node().is_some() => {
            let array = single.as_array_node()?;
            array.elements().iter().collect()
        }
        _ => arguments,
    };
    let mut elements = elements.into_iter();
    let template = elements.next()?;
    template.as_string_node()?;
    let value = elements.next();
    elements.next().is_none().then_some((template, value))
}

struct Patterns {
    eq_anonymous: Regex,
    in_anonymous: Regex,
    eq_named: Regex,
    in_named: Regex,
    is_null: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let compile = |source: &str| Regex::new(source).expect("static pattern");
        // Ruby's `\s` is ASCII only.
        let s = r"[ \t\r\n\f\x0B]";
        Patterns {
            eq_anonymous: compile(&format!(r"\A([\w.]+){s}+={s}+\?\z")),
            in_anonymous: compile(&format!(r"(?i)\A([\w.]+){s}+IN{s}+\(\?\)\z")),
            eq_named: compile(&format!(r"\A([\w.]+){s}+={s}+:(\w+)\z")),
            in_named: compile(&format!(r"(?i)\A([\w.]+){s}+IN{s}+\(:(\w+)\)\z")),
            is_null: compile(&format!(r"(?i)\A([\w.]+){s}+IS{s}+NULL\z")),
        }
    })
}

fn extract_column_and_value(
    ctx: &Context<'_>,
    template: &Node<'_>,
    value_node: Option<&Node<'_>>,
) -> Option<(String, String)> {
    let template = template.as_string_node()?;
    let template = std::str::from_utf8(template.unescaped()).ok()?;
    let patterns = patterns();
    let source = |node: &Node<'_>| String::from_utf8_lossy(ctx.text(node.span())).into_owned();

    let (column, value) = if let Some(captures) = patterns
        .eq_anonymous
        .captures(template)
        .or_else(|| patterns.in_anonymous.captures(template))
    {
        (captures[1].to_owned(), source(value_node?))
    } else if let Some(captures) =
        patterns.eq_named.captures(template).or_else(|| patterns.in_named.captures(template))
    {
        let hash = value_node?;
        let elements: Vec<Node<'_>> = if let Some(hash) = hash.as_keyword_hash_node() {
            hash.elements().iter().collect()
        } else {
            hash.as_hash_node()?.elements().iter().collect()
        };
        let wanted = captures[2].as_bytes();
        let pair = elements.iter().filter_map(Node::as_assoc_node).find(|pair| {
            let key = pair.key();
            if let Some(symbol) = key.as_symbol_node() {
                symbol.unescaped() == wanted
            } else {
                key.as_string_node().is_some_and(|string| string.unescaped() == wanted)
            }
        })?;
        // A shorthand pair's (`id:`) value is whitequark's `send`/`lvar`
        // named by the key, whose source omits the colon.
        let value = pair.value();
        let value = if value.as_implicit_node().is_some() {
            let key = ctx.text(pair.key().span());
            String::from_utf8_lossy(key.strip_suffix(b":").unwrap_or(key)).into_owned()
        } else {
            source(&value)
        };
        (captures[1].to_owned(), value)
    } else {
        let captures = patterns.is_null.captures(template)?;
        (captures[1].to_owned(), "nil".to_owned())
    };
    (column.matches('.').count() <= 1).then_some((column, value))
}

fn build_good_method(method_name: &str, column: &str, value: &str) -> String {
    if column.contains('.') {
        // Ruby's `String#split('.')` drops trailing empty fields.
        let mut parts: Vec<&str> = column.split('.').collect();
        while parts.last().is_some_and(|part| part.is_empty()) {
            parts.pop();
        }
        let table = parts.first().copied().unwrap_or("");
        let column = parts.get(1).copied().unwrap_or("");
        format!("{method_name}({table}: {{ {column}: {value} }})")
    } else {
        format!("{method_name}({column}: {value})")
    }
}
