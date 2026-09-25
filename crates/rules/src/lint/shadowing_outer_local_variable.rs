//! `Lint/ShadowingOuterLocalVariable`, ported from RuboCop's
//! `lib/rubocop/cop/lint/shadowing_outer_local_variable.rb`.
//!
//! RuboCop hooks `before_declaring_variable` and asks the variable table
//! what the name resolved to a moment earlier; [`ruby_semantic`] records
//! that answer on the variable itself ([`Variable::shadows`]), so the cop is
//! a single pass over every declaration.
//!
//! The two AST shapes that differ from whitequark's parser both show up in
//! `variable_node`: a Prism `BlockNode` hangs off its own `CallNode` instead
//! of owning it, and Prism spells parser's optional `begin` wrapper as a
//! `StatementsNode` (plus an `ElseNode` for `else` clauses). Both are
//! stepped over so `variable_node` lands on the node parser would report.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_semantic::{same, ScopeId, ScopeKind, Semantics, VariableId};
use ruby_source::Span;

/// `Lint::ShadowingOuterLocalVariable`.
#[derive(Debug, Clone)]
pub struct ShadowingOuterLocalVariable;

impl Rule for ShadowingOuterLocalVariable {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ShadowingOuterLocalVariable",
        department: Department::Lint,
        summary: "\
Do not use the same name as outer local variable for block arguments or \
block local variables.",
        explanation: "\
Checks for the use of local variable names from an outer scope in block
arguments or block-local variables. This mirrors the warning given by
`ruby -cw` prior to Ruby 2.6: \"shadowing outer local variable - foo\".

The cop is now disabled by default to match the upstream Ruby behavior.
It's useful, however, if you'd like to avoid shadowing variables from outer
scopes, which some people consider an anti-pattern that makes it harder to
keep track of what's going on in a program.

NOTE: Shadowing of variables in block passed to `Ractor.new` is allowed
because `Ractor` should not access outer variables.

```ruby
# bad
def some_method
  foo = 1

  2.times do |foo| # shadowing outer `foo`
    do_something(foo)
  end
end

# good
def some_method
  foo = 1

  2.times do |bar|
    do_something(bar)
  end
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ProgramNode],
        config: &[],
        blind_spots: "\
`same_conditions_node_different_branch?` compares the node that encloses
the shadowing block with the conditional that encloses the outer
declaration. Prism wraps clause bodies in a `StatementsNode` (and `else`
bodies in an `ElseNode`) where whitequark's parser emits the bare statement
or a `begin`; a single-statement `StatementsNode` is therefore treated as
transparent. A clause whose body is a parenthesised expression list
(`(a; b)`) keeps the extra `ParenthesesNode`, so the comparison can miss
there.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if node.kind() != NodeKind::ProgramNode {
            return;
        }
        let mut offenses: Vec<(Span, String)> = Vec::new();
        {
            let semantics = ctx.semantics();
            for id in semantics.variable_ids() {
                let variable = semantics.variable(id);
                if variable.should_be_unused() || is_ractor_block(semantics, id) {
                    continue;
                }
                let Some(outer) = variable.shadows() else { continue };
                if variable_used_in_declaration_of_outer(semantics, id, outer)
                    || same_conditions_node_different_branch(semantics, id, outer)
                {
                    continue;
                }
                let name = String::from_utf8_lossy(variable.name()).into_owned();
                offenses.push((variable.declaration().span(), name));
            }
        }
        for (span, name) in offenses {
            ctx.report(&Self::META, span, format!("Shadowing outer local variable - `{name}`."));
        }
    }
}

/// RuboCop's `ractor_block?`: `(block (send (const nil? :Ractor) :new ...)
/// ...)`. Prism makes the call the block's parent, so the receiver is read
/// from the scope's innermost ancestor.
fn is_ractor_block(semantics: &Semantics<'_>, id: VariableId) -> bool {
    let scope = semantics.variable(id).scope();
    if semantics.scope(scope).node().kind() != NodeKind::BlockNode {
        return false;
    }
    let Some(parent) = semantics.scope_ancestors(scope).last() else { return false };
    let Some(call) = parent.as_call_node() else { return false };
    if call.name().as_slice() != b"new" {
        return false;
    }
    call.receiver().is_some_and(|receiver| {
        receiver
            .as_constant_read_node()
            .is_some_and(|constant| constant.name().as_slice() == b"Ractor")
    })
}

/// RuboCop's `variable_used_in_declaration_of_outer?`.
fn variable_used_in_declaration_of_outer(
    semantics: &Semantics<'_>,
    id: VariableId,
    outer: VariableId,
) -> bool {
    let declaration = semantics.variable(outer).declaration();
    let scope = semantics.variable(id).scope();
    semantics.scope_ancestors(scope).iter().any(|ancestor| same(ancestor, &declaration))
}

/// RuboCop's `same_conditions_node_different_branch?`.
fn same_conditions_node_different_branch(
    semantics: &Semantics<'_>,
    id: VariableId,
    outer: VariableId,
) -> bool {
    let scope = semantics.variable(id).scope();
    let ancestors = semantics.scope_ancestors(scope);
    let Some((variable_node, variable_index)) = variable_node(semantics, scope) else {
        return false;
    };
    if !node_or_its_ascendant_conditional(&variable_node, &ancestors[..variable_index]) {
        return false;
    }

    let outer_ancestors = semantics.variable_ancestors(outer);
    let Some(outer_node) = find_conditional_node_from_ascendant(outer_ancestors) else {
        return false;
    };
    if same(&variable_node, &outer_node) {
        return true;
    }
    outer_node.kind() == NodeKind::IfNode
        && else_branch(&outer_node).is_some_and(|branch| same(&branch, &variable_node))
}

/// RuboCop's `variable_node`: the shadowing block's parent, with a `when`
/// clause standing in for its `case`. Returns the node and its index in the
/// scope's ancestor chain.
fn variable_node<'pr>(semantics: &Semantics<'pr>, scope: ScopeId) -> Option<(Node<'pr>, usize)> {
    let ancestors = semantics.scope_ancestors(scope);
    let mut index = ancestors.len();
    // A Prism block hangs off the call that takes it; parser makes the call
    // the block's own first child, so the call is not a parent there.
    if semantics.scope(scope).kind() == ScopeKind::Block {
        let owner = ancestors.get(index.checked_sub(1)?)?;
        if matches!(
            owner.kind(),
            NodeKind::CallNode | NodeKind::SuperNode | NodeKind::ForwardingSuperNode
        ) {
            index -= 1;
        }
    }
    loop {
        index = index.checked_sub(1)?;
        let node = ancestors[index];
        // A one-statement list is parser's bare statement; a longer one is
        // parser's `begin`. An `else` clause has no parser counterpart at
        // all.
        let transparent = node.kind() == NodeKind::ElseNode
            || (node.kind() == NodeKind::StatementsNode
                && node.as_statements_node()?.body().iter().count() == 1);
        if transparent {
            continue;
        }
        if node.kind() == NodeKind::WhenNode {
            let index = index.checked_sub(1)?;
            return Some((ancestors[index], index));
        }
        return Some((node, index));
    }
}

/// rubocop-ast's `Node#conditional?`.
fn is_conditional(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::WhileNode
            | NodeKind::UntilNode
            | NodeKind::CaseNode
            | NodeKind::CaseMatchNode
    )
}

/// RuboCop's `node_or_its_ascendant_conditional?`.
fn node_or_its_ascendant_conditional(node: &Node<'_>, ancestors: &[Node<'_>]) -> bool {
    is_conditional(node) || find_conditional_node_from_ascendant(ancestors).is_some()
}

/// RuboCop's `find_conditional_node_from_ascendant`: the innermost
/// conditional above a node.
fn find_conditional_node_from_ascendant<'pr>(ancestors: &[Node<'pr>]) -> Option<Node<'pr>> {
    ancestors.iter().rev().find(|node| is_conditional(node)).copied()
}

/// rubocop-ast's `IfNode#else_branch`. Prism's `ElseNode` wrapper and its
/// `StatementsNode` have no parser counterpart when the clause holds a
/// single statement.
fn else_branch<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    let subsequent = node.as_if_node()?.subsequent()?;
    let Some(clause) = subsequent.as_else_node() else {
        // `elsif`: parser nests another `if` node here too.
        return Some(subsequent);
    };
    let statements = clause.statements()?;
    let mut body = statements.body().iter();
    match (body.next(), body.next()) {
        (Some(only), None) => Some(only),
        (Some(_), Some(_)) => Some(statements.as_node()),
        _ => None,
    }
}
