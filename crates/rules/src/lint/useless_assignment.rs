//! `Lint/UselessAssignment`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_assignment.rb` plus the pieces it leans on:
//! the `VariableForce` model (here [`ruby_semantic`]), `IgnoredNode`
//! (`lib/rubocop/cop/ignored_node.rb`) for chained assignments, and
//! `RuboCop::NameSimilarity` (here [`crate::name_similarity`]) for the
//! ``Did you mean`` suffix.
//!
//! RuboCop hooks `after_leaving_scope`, so every scope is checked as the
//! `VariableForce` walk pops it -- inner scopes before the scopes that
//! contain them. That order is reproduced by iterating
//! [`ruby_semantic::Semantics::leave_order`] from this rule's single
//! `leave` on the root, because `IgnoredNode`'s byte-range suppression makes
//! the order observable.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_semantic::{same, AssignmentId, Semantics, VariableId};
use ruby_source::Span;

use crate::name_similarity::find_similar_name;

/// `Lint::UselessAssignment`.
#[derive(Debug, Clone)]
pub struct UselessAssignment;

impl Rule for UselessAssignment {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessAssignment",
        department: Department::Lint,
        summary: "Checks for useless assignment to a local variable.",
        explanation: "\
Checks for every useless assignment to local variable in every scope.
The basic idea for this cop was from the warning of `ruby -cw`:

```console
assigned but unused variable - foo
```

Currently this cop has advanced logic that detects unreferenced
reassignments and properly handles varied cases such as branch, loop,
rescue, ensure, etc.

This cop's autocorrection avoids cases like `a ||= 1` because removing
assignment from operator assignment can cause `NameError` if this
assignment has been used to declare a local variable.

NOTE: Given the assignment `foo = 1, bar = 2`, removing unused variables
can lead to a syntax error, so this case is not autocorrected.

```ruby
# bad
def some_method
  some_var = 1
  do_something
end

# good
def some_method
  some_var = 1
  do_something(some_var)
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ProgramNode],
        config: &[],
        blind_spots: "\
The `Did you mean` suffix reimplements Ruby's `DidYouMean::SpellChecker`
(Jaro-Winkler plus Levenshtein); `sort_by` ties inside the spell checker are
broken by insertion order rather than by Ruby's unstable sort, so a
dictionary holding two equally similar names may suggest the other one.

RuboCop reuses one cop object across autocorrection passes, so the byte
ranges `IgnoredNode` collects in one pass keep suppressing offenses in the
next; this port rebuilds rule state per pass, so a chained assignment such
as `foo = bar = do_something` keeps being simplified until nothing is left
to remove instead of stopping after one round.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if node.kind() != NodeKind::ProgramNode {
            return;
        }
        let mut reports: Vec<Report> = Vec::new();
        {
            let semantics = ctx.semantics();
            let mut ignored: Vec<Span> = Vec::new();
            for &scope in semantics.leave_order() {
                for &variable in semantics.scope(scope).variables() {
                    check_variable(semantics, ctx, variable, &mut ignored, &mut reports);
                }
            }
        }
        for report in reports {
            match report.fix {
                Some(fix) => ctx.report_with_fix(&Self::META, report.span, report.message, fix),
                None => ctx.report(&Self::META, report.span, report.message),
            }
        }
    }
}

/// One pending offense, held until the borrow of [`Context::semantics`] ends.
struct Report {
    span: Span,
    message: String,
    fix: Option<Fix>,
}

/// RuboCop's `check_for_unused_assignments`.
fn check_variable(
    semantics: &Semantics<'_>,
    ctx: &Context<'_>,
    variable: VariableId,
    ignored: &mut Vec<Span>,
    reports: &mut Vec<Report>,
) {
    if semantics.variable(variable).should_be_unused() {
        return;
    }
    let assignments = semantics.variable(variable).assignments().to_vec();
    for &assignment in assignments.iter().rev() {
        check_assignment(semantics, ctx, variable, assignment, ignored, reports);
    }
}

/// RuboCop's `check_for_unused_assignment`.
fn check_assignment(
    semantics: &Semantics<'_>,
    ctx: &Context<'_>,
    variable: VariableId,
    id: AssignmentId,
    ignored: &mut Vec<Span>,
    reports: &mut Vec<Report>,
) {
    let assignment = semantics.assignment(id);
    let node = assignment.node();
    let node_span = node.span();
    if semantics.assignment_used(id)
        || part_of_ignored_node(ignored, node_span)
        || variable_in_loop_condition(semantics, id, semantics.variable(variable).name())
    {
        return;
    }

    let name = String::from_utf8_lossy(semantics.variable(variable).name()).into_owned();
    let mut message = format!("Useless assignment to variable - `{name}`.");
    if let Some(extra) = message_specification(semantics, ctx, variable, id, &name) {
        message.push_str(&extra);
    }

    // `x = 1, y = 2` cannot lose a variable without becoming a syntax error,
    // and turning `x ||= 1` into `x || 1` can raise `NameError`.
    let fix = if sequential_assignment(semantics, id)
        || node.kind() == NodeKind::LocalVariableOrWriteNode
    {
        None
    } else {
        autocorrect(semantics, ctx, id)
    };

    reports.push(Report { span: assignment.name_span(), message, fix });

    if chained_assignment(&node) {
        ignored.push(node_span);
    }
}

/// `IgnoredNode#part_of_ignored_node?`, which compares byte ranges rather
/// than node identity.
fn part_of_ignored_node(ignored: &[Span], span: Span) -> bool {
    ignored.iter().any(|outer| outer.start <= span.start && outer.end >= span.end)
}

/// RuboCop's `message_specification`.
fn message_specification(
    semantics: &Semantics<'_>,
    ctx: &Context<'_>,
    variable: VariableId,
    id: AssignmentId,
    name: &str,
) -> Option<String> {
    let assignment = semantics.assignment(id);
    if assignment.is_multiple_assignment() {
        return Some(format!(
            " Use `_` or `_{name}` as a variable name to indicate that it won't be used."
        ));
    }
    if assignment.is_operator_assignment() {
        return operator_assignment_message(semantics, ctx, variable, id);
    }
    similar_name_message(semantics, variable, name)
}

/// RuboCop's `operator_assignment_message`: only when the operator
/// assignment is the scope's return value.
fn operator_assignment_message(
    semantics: &Semantics<'_>,
    ctx: &Context<'_>,
    variable: VariableId,
    id: AssignmentId,
) -> Option<String> {
    let assignment = semantics.assignment(id);
    let scope = semantics.scope(semantics.variable(variable).scope());
    let return_value = scope.return_value_node()?;
    if !same(&return_value, &assignment.node()) {
        return None;
    }
    let operator = String::from_utf8_lossy(ctx.text(assignment.operator_span()?)).into_owned();
    let base = operator.strip_suffix('=')?;
    Some(format!(" Use `{base}` instead of `{operator}`."))
}

/// RuboCop's `similar_name_message` over `collect_variable_like_names`.
fn similar_name_message(
    semantics: &Semantics<'_>,
    variable: VariableId,
    name: &str,
) -> Option<String> {
    let scope = semantics.scope(semantics.variable(variable).scope());
    let mut dictionary: Vec<String> = scope
        .bare_call_names()
        .iter()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .collect();
    for &id in scope.variables() {
        let other = String::from_utf8_lossy(semantics.variable(id).name()).into_owned();
        if !dictionary.contains(&other) {
            dictionary.push(other);
        }
    }
    find_similar_name(name, &dictionary).map(|similar| format!(" Did you mean `{similar}`?"))
}

/// RuboCop's `variable_in_loop_condition?`.
fn variable_in_loop_condition(semantics: &Semantics<'_>, id: AssignmentId, name: &[u8]) -> bool {
    let ancestors = semantics.assignment_ancestors(id);
    if ancestors.iter().any(|node| node.kind() == NodeKind::DefNode) {
        return false;
    }
    let loop_node = ancestors.iter().rev().find(|node| {
        matches!(node.kind(), NodeKind::WhileNode | NodeKind::UntilNode | NodeKind::ForNode)
    });
    let Some(loop_node) = loop_node else { return false };
    let condition = match loop_node.kind() {
        NodeKind::WhileNode => loop_node.as_while_node().expect("kind matched").predicate(),
        NodeKind::UntilNode => loop_node.as_until_node().expect("kind matched").predicate(),
        // `for` has no condition to read the variable from.
        _ => return false,
    };
    if reads_variable(&condition, name) {
        return true;
    }
    let mut found = false;
    each_descendant(&condition, &mut |node| {
        if reads_variable(node, name) {
            found = true;
        }
    });
    found
}

fn reads_variable(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_local_variable_read_node().is_some_and(|read| read.name().as_slice() == name)
}

/// RuboCop's `sequential_assignment?`: `foo = 1, bar = 2`, anywhere up the
/// ancestor chain.
fn sequential_assignment(semantics: &Semantics<'_>, id: AssignmentId) -> bool {
    let assignment = semantics.assignment(id);
    if is_sequential(&assignment.node()) {
        return true;
    }
    semantics.assignment_ancestors(id).iter().rev().any(is_sequential)
}

fn is_sequential(node: &Node<'_>) -> bool {
    let Some(write) = node.as_local_variable_write_node() else { return false };
    if write.value().kind() != NodeKind::ArrayNode {
        return false;
    }
    let mut found = false;
    each_descendant(node, &mut |descendant| {
        if is_assignment_node(descendant.kind()) {
            found = true;
        }
    });
    found
}

/// rubocop-ast's `Node#assignment?`.
fn is_assignment_node(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
    )
}

/// RuboCop's `chained_assignment?`.
fn chained_assignment(node: &Node<'_>) -> bool {
    node.as_local_variable_write_node().is_some_and(|write| {
        matches!(write.value().kind(), NodeKind::CallNode | NodeKind::LocalVariableWriteNode)
    })
}

/// RuboCop's `autocorrect`.
fn autocorrect(semantics: &Semantics<'_>, ctx: &Context<'_>, id: AssignmentId) -> Option<Fix> {
    let assignment = semantics.assignment(id);
    let node = assignment.node();
    let edit = if assignment.is_exception() {
        remove_exception_assignment_part(semantics, id)?
    } else if assignment.is_multiple_assignment()
        || assignment.is_rest_assignment()
        || assignment.is_for_assignment()
    {
        Edit::replace(node.span(), b"_".to_vec())
    } else if assignment.is_operator_assignment() {
        let operator = assignment.operator_span()?;
        Edit::delete(Span::new(operator.end - 1, operator.end))
    } else if assignment.is_regexp_named_capture() {
        let name = semantics.variable(assignment.variable()).name();
        replace_named_capture_group(ctx, assignment.name_span(), name)?
    } else {
        let value = node.as_local_variable_write_node()?.value();
        Edit::replace(node.span(), ctx.text(value.span()).to_vec())
    };
    Some(Fix { applicability: Applicability::Safe, edits: vec![edit] })
}

/// RuboCop's `remove_exception_assignment_part`: drops ` => error` from a
/// `rescue` clause.
fn remove_exception_assignment_part(semantics: &Semantics<'_>, id: AssignmentId) -> Option<Edit> {
    let assignment = semantics.assignment(id);
    let rescue = semantics.assignment_ancestors(id).last()?.as_rescue_node()?;
    let start = rescue
        .exceptions()
        .iter()
        .last()
        .map_or_else(|| rescue.keyword_loc().span().end, |node| node.span().end);
    Some(Edit::delete(Span::new(start, assignment.node().span().end)))
}

/// RuboCop's `replace_named_capture_group_with_non_capturing_group`.
fn replace_named_capture_group(ctx: &Context<'_>, regexp: Span, name: &[u8]) -> Option<Edit> {
    let source = ctx.text(regexp);
    let mut needle = Vec::with_capacity(name.len() + 4);
    needle.extend_from_slice(b"(?<");
    needle.extend_from_slice(name);
    needle.push(b'>');
    let at = source.windows(needle.len()).position(|window| window == needle.as_slice())?;
    let mut replaced = Vec::with_capacity(source.len());
    replaced.extend_from_slice(&source[..at]);
    replaced.extend_from_slice(b"(?:");
    replaced.extend_from_slice(&source[at + needle.len()..]);
    Some(Edit::replace(regexp, replaced))
}
