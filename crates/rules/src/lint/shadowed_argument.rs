//! `Lint/ShadowedArgument`, ported from RuboCop's
//! `lib/rubocop/cop/lint/shadowed_argument.rb`.
//!
//! RuboCop hooks `after_leaving_scope` via `VariableForce` (here
//! [`ruby_semantic`]) and, for every argument (method or block) in the
//! scope, looks for the first assignment that reassigns it without reading
//! its current value first. `def_node_search :uses_var?, '(lvar %)'` is a
//! raw AST search over the assignment's subtree (or, for a multiple/`for`/
//! rest assignment, the whole enclosing construct), reproduced here with
//! [`ruby_ast::each_descendant`] over `LocalVariableReadNode`.
//!
//! Two Prism/whitequark gaps matter here:
//! - whitequark's single `:rescue` node type (body + `resbody`s + `else`)
//!   splits into Prism's `BeginNode`/`RescueNode`/`ElseNode`; a `BeginNode`
//!   only stands in for it when it actually has a `rescue_clause`, since a
//!   bare `begin...end` (no rescue) is not conditional.
//! - `meta_assignment_node` (the whole `masgn`/`for`/bare-splat construct)
//!   is [`Meta::Multiple`]/[`Meta::For`]/[`Meta::Rest`]'s attached node,
//!   found in [`Semantics::assignment_ancestors`] rather than assumed to
//!   be the last entry, because a masgn rest target sits under an
//!   intermediate `SplatNode`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};
use ruby_semantic::{same, Meta, Semantics, Variable, VariableId};
use ruby_source::Span;

const MSG: &str = "Argument `%s` was shadowed by a local variable before it was used.";

/// `Lint::ShadowedArgument`.
#[derive(Debug, Clone)]
pub struct ShadowedArgument {
    ignore_implicit_references: bool,
}

impl Rule for ShadowedArgument {
    const META: RuleMeta = RuleMeta {
        name: "Lint/ShadowedArgument",
        department: Department::Lint,
        summary: "Avoid reassigning arguments before they were used.",
        explanation: "\
Checks for shadowed arguments.

This cop has `IgnoreImplicitReferences` configuration option. It means \
argument shadowing is used in order to pass parameters to zero arity \
`super` when `IgnoreImplicitReferences` is `true`.

```ruby
# bad
do_something do |foo|
  foo = 42
  puts foo
end

def do_something(foo)
  foo = 42
  puts foo
end

# good
do_something do |foo|
  foo = foo + 42
  puts foo
end

def do_something(foo)
  foo = foo + 42
  puts foo
end

def do_something(foo)
  puts foo
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[linter::ConfigOption {
            name: "IgnoreImplicitReferences",
            default: linter::ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Whether an implicit reference (a zero-arity `super` or a `binding` call) \
                  counts as a use of the argument before it is shadowed.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { ignore_implicit_references: options.bool("IgnoreImplicitReferences") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if node.kind() != NodeKind::ProgramNode {
            return;
        }
        let mut reports: Vec<(Span, String)> = Vec::new();
        {
            let semantics = ctx.semantics();
            for &scope in semantics.leave_order() {
                for &variable in semantics.scope(scope).variables() {
                    if let Some((span, name)) =
                        check_argument(semantics, variable, self.ignore_implicit_references)
                    {
                        reports.push((span, MSG.replace("%s", &String::from_utf8_lossy(name))));
                    }
                }
            }
        }
        for (span, message) in reports {
            ctx.report(&Self::META, span, message);
        }
    }
}

/// RuboCop's `check_argument`.
fn check_argument<'pr>(
    semantics: &Semantics<'pr>,
    id: VariableId,
    ignore_implicit_references: bool,
) -> Option<(Span, &'pr [u8])> {
    if !(semantics.is_method_argument(id) || semantics.is_block_argument(id)) {
        return None;
    }
    let variable = semantics.variable(id);
    // Block local variables, i.e., variables declared after `;` inside
    // `|...|`, aren't really arguments.
    if variable.is_explicit_block_local() {
        return None;
    }
    let span = shadowing_assignment(semantics, id, ignore_implicit_references)?;
    Some((span, variable.name()))
}

/// RuboCop's `shadowing_assignment`.
fn shadowing_assignment(
    semantics: &Semantics<'_>,
    id: VariableId,
    ignore_implicit_references: bool,
) -> Option<Span> {
    let variable = semantics.variable(id);
    if !variable.referenced() {
        return None;
    }

    let (candidate, location_known) = assignment_without_argument_usage(semantics, id)?;
    let assignment_without_usage_pos = candidate.span().start;

    let references = argument_references(semantics, variable);
    let shadowed = !references.into_iter().any(|reference| {
        if !reference.explicit() && ignore_implicit_references {
            return true;
        }
        reference_pos(&reference.node()) <= assignment_without_usage_pos
    });
    if !shadowed {
        return None;
    }

    Some(if location_known { candidate.span() } else { variable.declaration().span() })
}

/// RuboCop's `assignment_without_argument_usage`: the first assignment
/// whose enclosing construct doesn't read the argument's current value,
/// and whether that location is certain (not inside a branch/block/rescue
/// relative to the argument's scope).
fn assignment_without_argument_usage<'pr>(
    semantics: &Semantics<'pr>,
    id: VariableId,
) -> Option<(Node<'pr>, bool)> {
    let variable = semantics.variable(id);
    let scope_node = semantics.scope(variable.scope()).node();
    let mut location_known = true;

    for &aid in variable.assignments() {
        let assignment = semantics.assignment(aid);
        let ancestors = semantics.assignment_ancestors(aid);
        let (assignment_node, search_ancestors) = match assignment.meta() {
            Some(Meta::Multiple(wrap) | Meta::Rest(wrap) | Meta::For(wrap)) => {
                let idx = ancestors
                    .iter()
                    .position(|ancestor| same(ancestor, &wrap))
                    .expect("meta node is recorded among its own ancestors");
                (wrap, &ancestors[..idx])
            }
            _ => (assignment.node(), ancestors),
        };

        if is_shorthand_asgn(&assignment_node) {
            location_known = false;
            continue;
        }
        let Some(&parent) = search_ancestors.last() else {
            location_known = false;
            continue;
        };
        let conditional = conditional_assignment(search_ancestors, &scope_node);
        let _ = parent;

        if uses_var(&assignment_node, variable.name()) {
            continue;
        }
        if conditional {
            location_known = false;
            continue;
        }
        return Some((assignment.node(), location_known));
    }
    None
}

/// RuboCop's `reference_pos`.
fn reference_pos(node: &Node<'_>) -> u32 {
    node.span().start
}

/// RuboCop's `conditional_assignment?`, walking `search_ancestors`
/// (outermost first, ending at the assignment's immediate parent) inward
/// toward the root until `scope_node` is reached.
fn conditional_assignment(search_ancestors: &[Node<'_>], scope_node: &Node<'_>) -> bool {
    for ancestor in search_ancestors.iter().rev() {
        if same(ancestor, scope_node) {
            return false;
        }
        if is_conditional(ancestor) || is_block_or_rescue(ancestor) {
            return true;
        }
    }
    false
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

/// rubocop-ast's `Node#type?(:block, :rescue)`. A block/lambda body may not
/// run, and neither is a `begin`/`rescue` construct's body, handler(s) or
/// `else` clause guaranteed to complete; a bare `begin...end` without a
/// `rescue` is not conditional.
fn is_block_or_rescue(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::BlockNode | NodeKind::LambdaNode | NodeKind::RescueNode => true,
        NodeKind::BeginNode => {
            node.as_begin_node().is_some_and(|begin| begin.rescue_clause().is_some())
        }
        _ => false,
    }
}

/// RuboCop's `uses_var?`: `def_node_search :uses_var?, '(lvar %)'`, a raw
/// AST search for a local-variable read named `name` anywhere within
/// `node`'s subtree.
fn uses_var(node: &Node<'_>, name: &[u8]) -> bool {
    let mut found = false;
    each_descendant(node, &mut |descendant| {
        if found {
            return;
        }
        if descendant
            .as_local_variable_read_node()
            .is_some_and(|read| read.name().as_slice() == name)
        {
            found = true;
        }
    });
    found
}

/// RuboCop's `argument_references`: the variable's references, minus the
/// explicit ones that some assignment already recorded as reading its own
/// value (implicit references, from a zero-arity `super` or `binding`, are
/// always kept).
fn argument_references<'pr>(
    semantics: &Semantics<'pr>,
    variable: &Variable<'pr>,
) -> Vec<ruby_semantic::Reference<'pr>> {
    let assignment_reference_spans: Vec<Span> = variable
        .assignments()
        .iter()
        .flat_map(|&aid| semantics.assignment(aid).references().iter().map(Node::span))
        .collect();

    variable
        .references()
        .iter()
        .copied()
        .filter(|reference| {
            if !reference.explicit() {
                return true;
            }
            !assignment_reference_spans.contains(&reference.node().span())
        })
        .collect()
}

/// rubocop-ast's `Node#shorthand_asgn?`: `op_asgn`/`or_asgn`/`and_asgn`,
/// which in Prism are the local-variable operator/or/and write nodes (the
/// only shapes an argument's own reassignment can take without a meta
/// wrapper).
fn is_shorthand_asgn(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableAndWriteNode
    )
}
