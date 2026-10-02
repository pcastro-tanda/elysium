//! `Rails/CreateTableWithTimestamps`, ported from rubocop-rails'
//! `lib/rubocop/cop/rails/create_table_with_timestamps.rb` (with its
//! `ActiveRecordMigrationsHelper` mixin).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

const MSG: &str = "Add timestamps when creating a new table.";

/// Checks the migration for which timestamps are not included when creating a new table.
#[derive(Debug, Clone)]
pub struct CreateTableWithTimestamps;

impl Rule for CreateTableWithTimestamps {
    const META: RuleMeta = RuleMeta {
        name: "Rails/CreateTableWithTimestamps",
        department: Department::Rails,
        summary: "Checks the migration for which timestamps are not included when creating a new table.",
        explanation: "Checks the migration for which timestamps are not included when creating \
                      a new table. In many cases, timestamps are useful information and should \
                      be added.\n\nThe `id: false` option is respected: a table without a \
                      primary key is usually a join table.\n\n\
                      ```ruby\n# bad\ncreate_table :users\n\ncreate_table :users do |t|\n  t.string :name\n  t.string :email\nend\n\n\
                      # good\ncreate_table :users do |t|\n  t.string :name\n  t.string :email\n\n  t.timestamps\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
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
        if call.name().as_slice() != b"create_table"
            || call.receiver().is_some()
            || call.is_safe_navigation()
        {
            return;
        }
        let arguments: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        if arguments.iter().any(|argument| any_in_tree(argument, &is_id_false_pair)) {
            return;
        }
        let span = node.span();
        if let Some(block) = call.block().and_then(|block| block.as_block_node()) {
            let included = block.body().is_some_and(|body| time_columns_included(&body));
            if !included {
                ctx.report(&Self::META, span, MSG);
            }
        } else if !is_timestamps_proc(&call, &arguments) {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

/// `(send nil? :create_table (sym _) ... (block-pass (sym :timestamps)))`.
fn is_timestamps_proc(call: &ruby_ast::node::CallNode<'_>, arguments: &[Node<'_>]) -> bool {
    let Some(block) = call.block().and_then(|block| block.as_block_argument_node()) else {
        return false;
    };
    arguments.first().is_some_and(|first| first.as_symbol_node().is_some())
        && block
            .expression()
            .and_then(|expression| {
                expression.as_symbol_node().map(|s| s.unescaped() == b"timestamps")
            })
            .unwrap_or(false)
}

/// `node` or any descendant satisfies `predicate` (rubocop-ast's `def_node_search`).
fn any_in_tree(node: &Node<'_>, predicate: &dyn Fn(&Node<'_>) -> bool) -> bool {
    let mut found = predicate(node);
    if !found {
        each_descendant(node, &mut |child| found = found || predicate(child));
    }
    found
}

/// `(pair (sym :id) (false))`.
fn is_id_false_pair(node: &Node<'_>) -> bool {
    node.as_assoc_node().is_some_and(|pair| {
        pair.key().as_symbol_node().is_some_and(|key| key.unescaped() == b"id")
            && pair.value().as_false_node().is_some()
    })
}

fn time_columns_included(body: &Node<'_>) -> bool {
    any_in_tree(body, &|node| is_timestamps_call(node) || is_created_or_updated_at(node))
}

/// `(send _var :timestamps ...)`.
fn is_timestamps_call(node: &Node<'_>) -> bool {
    node.as_call_node()
        .is_some_and(|call| !call.is_safe_navigation() && call.name().as_slice() == b"timestamps")
}

/// `(send _var :datetime {(sym {:created_at :updated_at}) (str {"created_at" "updated_at"})} ...)`.
fn is_created_or_updated_at(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation() || call.name().as_slice() != b"datetime" {
        return false;
    }
    let Some(first) = call.arguments().and_then(|a| a.arguments().iter().next()) else {
        return false;
    };
    let name = if let Some(sym) = first.as_symbol_node() {
        sym.unescaped().to_vec()
    } else if let Some(string) = first.as_string_node() {
        string.unescaped().to_vec()
    } else {
        return false;
    };
    name == b"created_at" || name == b"updated_at"
}
