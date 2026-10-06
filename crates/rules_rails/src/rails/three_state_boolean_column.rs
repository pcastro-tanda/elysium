//! `Rails/ThreeStateBooleanColumn`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/three_state_boolean_column.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Boolean columns should always have a default value and a `NOT NULL` constraint.";

/// Enforces that boolean columns are created with default values (`false` or
/// `true`) and `NOT NULL` constraint.
#[derive(Debug, Clone)]
pub struct ThreeStateBooleanColumn {
    /// `def`/`defs` nodes seen so far with the `change_column_null` calls
    /// (`table`, `column`) setting a column to `NOT NULL` inside each.
    defs: Vec<(Span, Vec<(String, String)>)>,
    /// `create_table`/`change_table` calls with a literal block, with the
    /// first argument of the call.
    table_blocks: Vec<(Span, Option<String>)>,
}

impl Rule for ThreeStateBooleanColumn {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ThreeStateBooleanColumn",
        department: Department::Rails,
        summary: "Add a default value and a `NOT NULL` constraint to boolean columns.",
        explanation: "Enforces that boolean columns are created with default values (`false` or \
                      `true`) and `NOT NULL` constraint.\n\n```ruby\n# bad\nadd_column :users, \
                      :active, :boolean\nt.column :active, :boolean\nt.boolean :active\n\n# \
                      good\nadd_column :users, :active, :boolean, default: true, null: false\n\
                      t.column :active, :boolean, default: true, null: false\nt.boolean :active, \
                      default: true, null: false\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { defs: Vec::new(), table_blocks: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(def) = node.as_def_node() {
            let mut calls = Vec::new();
            if let Some(body) = def.body() {
                each_descendant(&body, &mut |child| {
                    if let Some((table, column)) = change_column_null(ctx, child) {
                        calls.push((table, column));
                    }
                });
            }
            self.defs.push((node.span(), calls));
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let name = call.name();
        if matches!(name.as_slice(), b"create_table" | b"change_table")
            && call.block().is_some_and(|block| {
                block.as_block_node().is_some_and(|b| {
                    !b.parameters().is_some_and(|p| {
                        p.as_numbered_parameters_node().is_some()
                            || p.as_it_parameters_node().is_some()
                    })
                })
            })
        {
            let first = call.arguments().and_then(|a| a.arguments().iter().next());
            self.table_blocks.push((node.span(), first.map(|a| key(ctx, &a))));
        }
        if call.is_safe_navigation() {
            return;
        }
        let mut args: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        if let Some(block) = call.block() {
            if block.as_block_argument_node().is_some() {
                args.push(block);
            }
        }
        let has_receiver = call.receiver().is_some();
        let (table, column, options) = match name.as_slice() {
            b"add_column" if !has_receiver => {
                let [table, column, kind] = &args[..args.len().min(3)] else { return };
                if !is_boolean_type(kind) || args.len() > 4 {
                    return;
                }
                (Some(key(ctx, table)), column, args.get(3))
            }
            b"column" if has_receiver => {
                let [column, kind] = &args[..args.len().min(2)] else { return };
                if !is_boolean_type(kind) || args.len() > 3 {
                    return;
                }
                (self.table_of_block(node.span()), column, args.get(2))
            }
            b"boolean" if has_receiver => {
                if args.is_empty() || args.len() > 2 {
                    return;
                }
                (self.table_of_block(node.span()), &args[0], args.get(1))
            }
            _ => return,
        };
        if options.is_some_and(required_options) {
            return;
        }
        let enclosing_def = self.defs.iter().rev().find(|(span, _)| contains(*span, node.span()));
        if let Some((_, calls)) = enclosing_def {
            let column_key = key(ctx, column);
            match table {
                None => return,
                Some(table_key) => {
                    if calls.iter().any(|(t, c)| *t == table_key && *c == column_key) {
                        return;
                    }
                }
            }
        }
        ctx.report(&Self::META, call_span_excluding_block(&call), MSG);
    }
}

impl ThreeStateBooleanColumn {
    /// The first argument of the nearest enclosing `create_table` or
    /// `change_table` block, if any.
    fn table_of_block(&self, span: Span) -> Option<String> {
        self.table_blocks
            .iter()
            .rev()
            .find(|(block, _)| contains(*block, span))
            .and_then(|(_, table)| table.clone())
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// `{(sym :boolean) (str "boolean")}`.
fn is_boolean_type(node: &Node<'_>) -> bool {
    node.as_symbol_node().is_some_and(|s| s.unescaped() == b"boolean")
        || node.as_string_node().is_some_and(|s| s.unescaped() == b"boolean")
}

/// `(hash <(pair (sym :default) !nil?) (pair (sym :null) false) ...>)`.
fn required_options(node: &Node<'_>) -> bool {
    let elements = if let Some(hash) = node.as_keyword_hash_node() {
        hash.elements()
    } else if let Some(hash) = node.as_hash_node() {
        hash.elements()
    } else {
        return false;
    };
    let pair_with = |name: &[u8], accept: &dyn Fn(&Node<'_>) -> bool| {
        elements.iter().any(|element| {
            element.as_assoc_node().is_some_and(|pair| {
                pair.key().as_symbol_node().is_some_and(|k| k.unescaped() == name)
                    && accept(&pair.value())
            })
        })
    };
    pair_with(b"default", &|value| value.as_nil_node().is_none())
        && pair_with(b"null", &|value| value.as_false_node().is_some())
}

/// `(send nil? :change_column_null table column false)`: the keys of
/// `table` and `column`.
fn change_column_null(ctx: &Context<'_>, node: &Node<'_>) -> Option<(String, String)> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"change_column_null"
        || call.receiver().is_some()
        || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
    {
        return None;
    }
    let arguments = call.arguments()?;
    let list: Vec<Node<'_>> = arguments.arguments().iter().collect();
    let [table, column, flag] = &list[..] else { return None };
    flag.as_false_node()?;
    Some((key(ctx, table), key(ctx, column)))
}

/// A comparable form of a node: symbols and strings by value, anything else
/// by source.
fn key(ctx: &Context<'_>, node: &Node<'_>) -> String {
    if let Some(symbol) = node.as_symbol_node() {
        format!("sym:{}", String::from_utf8_lossy(symbol.unescaped()))
    } else if let Some(string) = node.as_string_node() {
        format!("str:{}", String::from_utf8_lossy(string.unescaped()))
    } else {
        format!("src:{}", String::from_utf8_lossy(ctx.text(node.span())))
    }
}
