//! `Rails/ReversibleMigration`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/reversible_migration.rb` (with its
//! `MigrationsHelper` mixin, `lib/rubocop/cop/mixin/migrations_helper.rb`).

use std::collections::HashSet;

use linter::{
    Context, Department, FixAvailability, OptionError, OptionValue, Rule, RuleMeta, RuleOptions,
    Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::node::{CallNode, ClassNode};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `TargetRailsVersion::DEFAULT_RAILS_VERSION`, used when the configuration
/// states none.
const DEFAULT_RAILS_VERSION: f64 = 5.0;

/// Checks whether the change method of the migration file is reversible.
#[derive(Debug, Clone)]
pub struct ReversibleMigration {
    target_rails_version: f64,
    /// Enclosing `ActiveRecord::Migration[x.y]` classes.
    migration_depth: u32,
    /// Enclosing `def change` methods.
    change_depth: u32,
    /// Enclosing `reversible { }` / `up_only { }` blocks.
    reversible_depth: u32,
    /// Offense ranges already registered: RuboCop registers one offense per
    /// range (`Base#add_offense`).
    seen: HashSet<Span>,
}

impl Rule for ReversibleMigration {
    const META: RuleMeta = RuleMeta {
        name: "Rails/ReversibleMigration",
        department: Department::Rails,
        summary: "Checks whether the change method of the migration file is reversible.",
        explanation: "Checks whether the change method of the migration file is reversible.\n\n\
                      ```ruby\n# bad\ndef change\n  change_table :users do |t|\n    t.remove \
                      :name\n  end\nend\n\n# good\ndef change\n  change_table :users do |t|\n    \
                      t.remove :name, type: :string\n  end\nend\n\n# good\ndef change\n  \
                      create_table :users do |t|\n    t.string :name\n  end\nend\n\n# bad\ndef \
                      change\n  drop_table :users\nend\n\n# good\ndef change\n  drop_table \
                      :users do |t|\n    t.string :name\n  end\nend\n\n# bad\ndef change\n  \
                      remove_column(:suppliers, :qualification)\nend\n\n# good\ndef change\n  \
                      remove_column(:suppliers, :qualification, :string)\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode, NodeKind::DefNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be 5.0; \
                      RuboCop reads `railties` from the project's `Gemfile.lock` first. Inside \
                      `change_table`, only sends that are statements of the block (or the sole \
                      statement's call) are inspected, not those nested in other constructs.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            target_rails_version: target_rails_version(options),
            migration_depth: 0,
            change_depth: 0,
            reversible_depth: 0,
            seen: HashSet::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ClassNode => {
                if node.as_class_node().is_some_and(|class| is_migration_class(&class)) {
                    self.migration_depth += 1;
                }
            }
            NodeKind::DefNode => {
                if is_change_def(node) {
                    self.change_depth += 1;
                }
            }
            NodeKind::CallNode => {
                let Some(call) = node.as_call_node() else { return };
                if self.migration_depth > 0 && self.change_depth > 0 && self.reversible_depth == 0 {
                    self.check_block(&call, ctx);
                    self.check_send(&call, ctx);
                }
                if opens_reversible_block(&call) {
                    self.reversible_depth += 1;
                }
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ClassNode => {
                if node.as_class_node().is_some_and(|class| is_migration_class(&class)) {
                    self.migration_depth -= 1;
                }
            }
            NodeKind::DefNode => {
                if is_change_def(node) {
                    self.change_depth -= 1;
                }
            }
            NodeKind::CallNode => {
                if node.as_call_node().is_some_and(|call| opens_reversible_block(&call)) {
                    self.reversible_depth -= 1;
                }
            }
            _ => {}
        }
    }
}

impl ReversibleMigration {
    fn offense(&mut self, ctx: &mut Context<'_>, span: Span, action: &str) {
        if self.seen.insert(span) {
            ctx.report(&Self::META, span, format!("{action} is not reversible."));
        }
    }

    fn at_least_6_1(&self) -> bool {
        self.target_rails_version >= 6.1
    }

    /// `on_block` (also `on_numblock` / `on_itblock`).
    fn check_block(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return };
        let Some(body) = block.body() else { return };
        // `(send nil? :change_table $_ ...)`.
        if call.receiver().is_some()
            || call.is_safe_navigation()
            || call.name().as_slice() != b"change_table"
        {
            return;
        }
        let arguments = parser_arguments(call);
        let Some(table) = arguments.first() else { return };
        let table_text = ctx.text(table.span()).to_vec();

        let mut children = Vec::new();
        let Some(statements) = body.as_statements_node() else { return };
        let statements: Vec<Node<'_>> = statements.body().iter().collect();
        if let [only] = statements.as_slice() {
            // `block.send_type?`, else the block-typed child's `send`.
            if let Some(send) = only.as_call_node() {
                if send.block().is_some_and(|block| block.as_block_node().is_some()) {
                    if !send.is_safe_navigation() {
                        children.push(send);
                    }
                } else if !send.is_safe_navigation() {
                    children.push(send);
                }
            }
        } else {
            for statement in &statements {
                if let Some(send) = statement.as_call_node() {
                    if !send.is_safe_navigation()
                        && !send.block().is_some_and(|block| block.as_block_node().is_some())
                    {
                        children.push(send);
                    }
                }
            }
        }
        for child in &children {
            self.check_change_table_offense(&table_text, child, ctx);
        }
    }

    fn check_change_table_offense(
        &mut self,
        table_text: &[u8],
        node: &CallNode<'_>,
        ctx: &mut Context<'_>,
    ) {
        let same_receiver =
            node.receiver().is_some_and(|receiver| ctx.text(receiver.span()) == table_text);
        if !same_receiver && self.reversible_change_table_call(node) {
            return;
        }
        let name = String::from_utf8_lossy(node.name().as_slice()).into_owned();
        let action = if name == "remove" {
            if self.at_least_6_1() { "t.remove (without type)" } else { "t.remove" }.to_owned()
        } else {
            format!("change_table(with {name})")
        };
        self.offense(ctx, call_span_excluding_block(node), &action);
    }

    /// `on_send`.
    fn check_send(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if call.is_safe_navigation() {
            return;
        }
        let span = call_span_excluding_block(call);
        let arguments = parser_arguments(call);
        let bare = call.receiver().is_none();
        let name = call.name();
        let name = name.as_slice();

        // check_irreversible_schema_statement_node
        if bare && (name == b"change_column" || name == b"execute") {
            self.offense(ctx, span, &String::from_utf8_lossy(name));
        }

        // check_drop_table_node
        if bare && name == b"drop_table" && !arguments.is_empty() {
            let block_pass = call.block().is_some_and(|b| b.as_block_argument_node().is_some());
            if !(parent_is_block(call, ctx) || block_pass) {
                self.offense(ctx, span, "drop_table(without block)");
            }
        }

        // check_reversible_hash_node
        if !self.reversible_change_table_call(call) {
            let action = format!("{}(without :from and :to)", String::from_utf8_lossy(name));
            self.offense(ctx, span, &action);
        }

        // check_remove_column_node
        if bare && name == b"remove_column" && arguments.len() < 3 {
            self.offense(ctx, span, "remove_column(without type)");
        }

        // check_remove_foreign_key_node
        if bare && name == b"remove_foreign_key" {
            if let [_, last] = arguments.as_slice() {
                if is_hash(last) && !all_hash_key(Some(last), &[b"to_table"]) {
                    self.offense(ctx, span, "remove_foreign_key(without table)");
                }
            }
        }

        // check_remove_columns_node
        if bare && name == b"remove_columns" {
            if let Some(last) = arguments.last() {
                if !(all_hash_key(Some(last), &[b"type"]) && self.at_least_6_1()) {
                    let action = if self.at_least_6_1() {
                        "remove_columns(without type)"
                    } else {
                        "remove_columns"
                    };
                    self.offense(ctx, span, action);
                }
            }
        }

        // check_remove_index_node
        if bare && name == b"remove_index" {
            if let [_, last] = arguments.as_slice() {
                if is_hash(last) && !all_hash_key(Some(last), &[b"column"]) {
                    self.offense(ctx, span, "remove_index(without column)");
                }
            }
        }
    }

    fn reversible_change_table_call(&self, node: &CallNode<'_>) -> bool {
        let arguments = parser_arguments(node);
        let last = arguments.last();
        match node.name().as_slice() {
            b"change" => false,
            b"remove" => self.at_least_6_1() && all_hash_key(last, &[b"type"]),
            b"remove_index" => {
                arguments.iter().any(|arg| !is_hash(arg)) || all_hash_key(last, &[b"column"])
            }
            b"change_default"
            | b"change_column_default"
            | b"change_table_comment"
            | b"change_column_comment" => all_hash_key(last, &[b"from", b"to"]),
            _ => true,
        }
    }
}

/// `node.arguments`: a `&block` argument counts as the last argument.
fn parser_arguments<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut arguments: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block() {
        if block.as_block_argument_node().is_some() {
            arguments.push(block);
        }
    }
    arguments
}

fn is_hash(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode)
}

/// `all_hash_key?`: a hash argument whose keys include every one of `keys`.
fn all_hash_key(args: Option<&Node<'_>>, keys: &[&[u8]]) -> bool {
    let Some(args) = args else { return false };
    let elements: Vec<Node<'_>> = if let Some(hash) = args.as_hash_node() {
        hash.elements().iter().collect()
    } else if let Some(hash) = args.as_keyword_hash_node() {
        hash.elements().iter().collect()
    } else {
        return false;
    };
    let hash_keys: Vec<Vec<u8>> = elements
        .iter()
        .filter_map(|element| {
            let key = element.as_assoc_node()?.key();
            if let Some(symbol) = key.as_symbol_node() {
                Some(symbol.unescaped().to_vec())
            } else {
                key.as_string_node().map(|string| string.unescaped().to_vec())
            }
        })
        .collect();
    keys.iter().all(|key| hash_keys.iter().any(|hash_key| hash_key == key))
}

/// `node.parent.any_block_type?` for a send: it carries a literal block, or
/// is the sole statement of a block or lambda body.
fn parent_is_block(call: &CallNode<'_>, ctx: &Context<'_>) -> bool {
    if call.block().is_some_and(|block| block.as_block_node().is_some()) {
        return true;
    }
    let ancestors = ctx.ancestors();
    let [.., grandparent, parent] = ancestors else { return false };
    let span = call.as_node().span();
    parent.kind == NodeKind::StatementsNode
        && parent.span == span
        && matches!(grandparent.kind, NodeKind::BlockNode | NodeKind::LambdaNode)
}

/// A `reversible`/`up_only` call with a literal block that takes no numbered
/// or `it` parameters (RuboCop's `:block` type).
fn opens_reversible_block(call: &CallNode<'_>) -> bool {
    let name = call.name();
    if name.as_slice() != b"reversible" && name.as_slice() != b"up_only" {
        return false;
    }
    let Some(block) = call.block().and_then(|block| block.as_block_node()) else { return false };
    !block.parameters().is_some_and(|parameters| {
        matches!(parameters.kind(), NodeKind::NumberedParametersNode | NodeKind::ItParametersNode)
    })
}

/// `def change` (not `def self.change`).
fn is_change_def(node: &Node<'_>) -> bool {
    node.as_def_node()
        .is_some_and(|def| def.receiver().is_none() && def.name().as_slice() == b"change")
}

/// `MigrationsHelper#migration_class?`:
/// `(class (const {nil? cbase} _) (send (const (const {nil? cbase} :ActiveRecord) :Migration) :[] (float _)) _)`.
fn is_migration_class(class: &ClassNode<'_>) -> bool {
    if !is_bare_or_toplevel_const(&class.constant_path()) {
        return false;
    }
    let Some(superclass) = class.superclass() else { return false };
    let Some(call) = superclass.as_call_node() else { return false };
    if call.name().as_slice() != b"[]" || call.is_safe_navigation() {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    let Some(path) = receiver.as_constant_path_node() else { return false };
    if path.name().is_none_or(|name| name.as_slice() != b"Migration") {
        return false;
    }
    let active_record = path.parent().is_some_and(|parent| {
        is_bare_or_toplevel_const(&parent) && const_name(&parent).as_deref() == Some("ActiveRecord")
    });
    if !active_record || call.block().is_some() {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let mut arguments = arguments.arguments().iter();
    matches!((arguments.next(), arguments.next()), (Some(arg), None) if arg.as_float_node().is_some())
}

/// `Config#target_rails_version`: `AllCops/TargetRailsVersion` when set.
fn target_rails_version(options: &RuleOptions) -> f64 {
    match options.peer("AllCops", "TargetRailsVersion") {
        Some(OptionValue::Str(text)) => text.trim().parse().unwrap_or(DEFAULT_RAILS_VERSION),
        Some(value) => value.as_float().unwrap_or(DEFAULT_RAILS_VERSION),
        None => DEFAULT_RAILS_VERSION,
    }
}
