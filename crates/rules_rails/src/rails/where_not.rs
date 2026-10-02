//! `Rails/WhereNot`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/where_not.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

// Ruby's `\w` and `\s` are ASCII-only, unlike the `regex` crate's.
/// `column != ?`, `column <> ?`.
const NOT_EQ_ANONYMOUS: &str =
    r"\A([A-Za-z0-9_.]+)[ \t\r\n\x0c\x0b]+(?:!=|<>)[ \t\r\n\x0c\x0b]+\?\z";
/// `column NOT IN (?)`.
const NOT_IN_ANONYMOUS: &str =
    r"(?i)\A([A-Za-z0-9_.]+)[ \t\r\n\x0c\x0b]+NOT[ \t\r\n\x0c\x0b]+IN[ \t\r\n\x0c\x0b]+\(\?\)\z";
/// `column != :column`, `column <> :column`.
const NOT_EQ_NAMED: &str =
    r"\A([A-Za-z0-9_.]+)[ \t\r\n\x0c\x0b]+(?:!=|<>)[ \t\r\n\x0c\x0b]+:([A-Za-z0-9_]+)\z";
/// `column NOT IN (:column)`.
const NOT_IN_NAMED: &str = r"(?i)\A([A-Za-z0-9_.]+)[ \t\r\n\x0c\x0b]+NOT[ \t\r\n\x0c\x0b]+IN[ \t\r\n\x0c\x0b]+\(:([A-Za-z0-9_]+)\)\z";
/// `column IS NOT NULL`.
const IS_NOT_NULL: &str =
    r"(?i)\A([A-Za-z0-9_.]+)[ \t\r\n\x0c\x0b]+IS[ \t\r\n\x0c\x0b]+NOT[ \t\r\n\x0c\x0b]+NULL\z";

/// Identifies places where manually constructed SQL in `where` can be
/// replaced with `where.not(...)`.
#[derive(Debug, Clone)]
pub struct WhereNot {
    not_eq_anonymous: Regex,
    not_in_anonymous: Regex,
    not_eq_named: Regex,
    not_in_named: Regex,
    is_not_null: Regex,
}

impl Rule for WhereNot {
    const META: RuleMeta = RuleMeta {
        name: "Rails/WhereNot",
        department: Department::Rails,
        summary: "Use `where.not(...)` instead of manually constructing negated SQL in `where`.",
        explanation: "Identifies places where manually constructed SQL in `where` can be \
                      replaced with `where.not(...)`.\n\n```ruby\n# bad\nUser.where('name != \
                      ?', 'Gabe')\nUser.where('name != :name', name: 'Gabe')\nUser.where('name \
                      <> ?', 'Gabe')\nUser.where('name <> :name', name: \
                      'Gabe')\nUser.where('name IS NOT NULL')\nUser.where('name NOT IN (?)', \
                      ['john', 'jane'])\nUser.where('name NOT IN (:names)', names: ['john', \
                      'jane'])\nUser.where('users.name != :name', name: 'Gabe')\n\n# \
                      good\nUser.where.not(name: 'Gabe')\nUser.where.not(name: \
                      nil)\nUser.where.not(name: ['john', 'jane'])\nUser.where.not(users: { \
                      name: 'Gabe' })\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        let compile = |pattern: &str| Regex::new(pattern).expect("static pattern");
        Ok(Self {
            not_eq_anonymous: compile(NOT_EQ_ANONYMOUS),
            not_in_anonymous: compile(NOT_IN_ANONYMOUS),
            not_eq_named: compile(NOT_EQ_NAMED),
            not_in_named: compile(NOT_IN_NAMED),
            is_not_null: compile(IS_NOT_NULL),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"where" {
            return;
        }
        let Some(selector) = call.message_loc() else { return };

        // whitequark's `send` arguments, a `&block` argument included.
        let mut arguments: Vec<Node<'_>> =
            call.arguments().map_or_else(Vec::new, |args| args.arguments().iter().collect());
        let literal_block = call.block().is_some_and(|block| block.as_block_node().is_some());
        if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
            arguments.push(block);
        }

        // `(call _ :where (array $str_type? $_ ?))` | `(call _ :where $str_type? $_ ?)`.
        let (template, value_node) = match arguments.as_slice() {
            [only] if only.as_array_node().is_some() => {
                let array = only.as_array_node().expect("checked");
                let elements: Vec<Node<'_>> = array.elements().iter().collect();
                match elements.as_slice() {
                    [template] => (template.as_string_node(), None),
                    [template, value] => (template.as_string_node(), Some(*value)),
                    _ => return,
                }
            }
            [template] => (template.as_string_node(), None),
            [template, value] => (template.as_string_node(), Some(*value)),
            _ => return,
        };
        let Some(template) = template else { return };
        let Ok(template) = std::str::from_utf8(template.unescaped()) else { return };

        let Some((column, value)) =
            self.extract_column_and_value(ctx, template, value_node.as_ref())
        else {
            return;
        };

        // `node.loc.selector.begin_pos` to the end of the `send` (a literal
        // block is a separate node in whitequark).
        let end = if literal_block {
            match call.closing_loc() {
                Some(closing) => closing.span().end,
                None => arguments.last().map_or(node.span().end, |last| last.span().end),
            }
        } else {
            node.span().end
        };
        let range = Span::new(selector.span().start, end);

        let dot = call.call_operator_loc().map_or(".".to_owned(), |loc| {
            String::from_utf8_lossy(ctx.text(loc.span())).into_owned()
        });
        let good_method = build_good_method(&dot, column, &value);
        let message =
            format!("Use `{good_method}` instead of manually constructing negated SQL in `where`.");
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(range, good_method.into_bytes())],
            },
        );
    }
}

impl WhereNot {
    /// `extract_column_and_value`: the column qualifier and replacement value
    /// source, or `None` when the template does not qualify.
    fn extract_column_and_value<'a>(
        &self,
        ctx: &Context<'_>,
        template: &'a str,
        value_node: Option<&Node<'_>>,
    ) -> Option<(&'a str, String)> {
        let source = |node: &Node<'_>| String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let (qualifier, value) = if let Some(captures) = self
            .not_eq_anonymous
            .captures(template)
            .or_else(|| self.not_in_anonymous.captures(template))
        {
            (captures.get(1)?.as_str(), value_node.map(source)?)
        } else if let Some(captures) =
            self.not_eq_named.captures(template).or_else(|| self.not_in_named.captures(template))
        {
            let name = captures.get(2)?.as_str().as_bytes();
            let value = value_node?;
            let elements = match value.kind() {
                NodeKind::HashNode => value.as_hash_node()?.elements(),
                NodeKind::KeywordHashNode => value.as_keyword_hash_node()?.elements(),
                _ => return None,
            };
            let pair =
                elements.iter().filter_map(|element| element.as_assoc_node()).find(|pair| {
                    let key = pair.key();
                    key.as_symbol_node()
                        .map(|symbol| symbol.unescaped().to_vec())
                        .or_else(|| key.as_string_node().map(|string| string.unescaped().to_vec()))
                        .is_some_and(|key| key == name)
                })?;
            let pair_value = pair.value();
            let text = if pair_value.as_implicit_node().is_some() {
                let key = ctx.text(pair.key().span());
                String::from_utf8_lossy(key.strip_suffix(b":").unwrap_or(key)).into_owned()
            } else {
                source(&pair_value)
            };
            (captures.get(1)?.as_str(), text)
        } else {
            let captures = self.is_not_null.captures(template)?;
            (captures.get(1)?.as_str(), "nil".to_owned())
        };
        if qualifier.bytes().filter(|&b| b == b'.').count() > 1 {
            return None;
        }
        Some((qualifier, value))
    }
}

/// `build_good_method`.
fn build_good_method(dot: &str, column: &str, value: &str) -> String {
    if column.contains('.') {
        // Ruby's `String#split` drops trailing empty fields.
        let mut parts: Vec<&str> = column.split('.').collect();
        while parts.last().is_some_and(|part| part.is_empty()) {
            parts.pop();
        }
        let table = parts.first().copied().unwrap_or("");
        let column = parts.get(1).copied().unwrap_or("");
        format!("where{dot}not({table}: {{ {column}: {value} }})")
    } else {
        format!("where{dot}not({column}: {value})")
    }
}
