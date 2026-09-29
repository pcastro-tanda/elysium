//! `Style/EachWithObject`, ported from RuboCop's
//! `lib/rubocop/cop/style/each_with_object.rb`.
//!
//! # Matched shape
//!
//! Upstream's two node-matcher patterns (`each_with_object_block_candidate?`
//! on `(block (call _ {:inject :reduce} _) (args $_ $_) $_)` and
//! `each_with_object_numblock_candidate?` on `(numblock (call _ {:inject
//! :reduce} _) 2 $_)`) both require the `inject`/`reduce` dispatch itself to
//! carry *exactly one* argument -- a bare `.inject { ... }` or a two-argument
//! `.inject(0, :+) { ... }` fail the pattern outright, matching this port's
//! `arguments().len() != 1` early return (`does not blow up for reduce with
//! no arguments`). The pattern's `call` node type (not `send`) already
//! includes `&.`, so `is_safe_navigation` is never checked here (`finds
//! inject is safe navigation called ...`).
//!
//! Both patterns key off the *literal* type Prism gives a block's own
//! `parameters()`: [`ruby_ast::node::BlockParametersNode`] for `|a, e|`,
//! [`ruby_ast::node::NumberedParametersNode`] for bare `_1`/`_2`. Prism's
//! separate `ItParametersNode` for a Ruby-3.4 `it` block naturally falls
//! through neither arm, matching upstream (only `on_block`/`on_numblock` are
//! wired up; `on_itblock` is not aliased -- `does not register an offense
//! when using itblock with inject`).
//!
//! # `accumulator_param_assigned_to?`
//!
//! Upstream detects a reassigned accumulator by identity-comparing every
//! descendant assignment node's own bare-symbol left-hand side against the
//! first block parameter's name; for `+=`/`||=`/`&&=`/multiple assignment,
//! whitequark's `op_asgn`/`or_asgn`/`and_asgn`/`masgn` wrap a *plain*
//! `lvasgn` (or `mlhs` of them) as their own left-hand-side child, so the
//! match only ever fires on that nested plain-assignment descendant, never
//! on the compound node itself. Prism instead gives each simple/compound
//! local-variable write its own node kind carrying the name directly
//! (`LocalVariableWriteNode`, `LocalVariableOperatorWriteNode`,
//! `LocalVariableAndWriteNode`, `LocalVariableOrWriteNode`) plus
//! `LocalVariableTargetNode` for each name in a multi-assignment's target
//! list, so [`assigns_to`] checks those five kinds directly for the same net
//! effect, with no plain-`lvasgn`-child detour needed.
//!
//! # Autocorrect
//!
//! [`autocorrect_block`] swaps the two block parameters' source text and
//! deletes the trailing `accumulator` return statement -- the whole line
//! (RuboCop's `range_by_whole_lines(..., include_final_newline: true)`) when
//! it is the only thing on that line, or just its own span otherwise.
//! [`autocorrect_numblock`] never deletes anything (upstream's "avoid a
//! clobbering error" comment): it swaps every `_1`/`_2`
//! [`ruby_ast::node::LocalVariableReadNode`] reference in the body instead,
//! including the trailing return reference itself.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::BlockNode;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{is_ruby_whitespace, Span};

/// Prefer `each_with_object` over `inject` or `reduce`.
#[derive(Debug, Clone)]
pub struct EachWithObject;

impl Rule for EachWithObject {
    const META: RuleMeta = RuleMeta {
        name: "Style/EachWithObject",
        department: Department::Style,
        summary: "Prefer `each_with_object` over `inject` or `reduce`.",
        explanation: "\
Looks for inject / reduce calls where the passed in object is
returned at the end and so could be replaced by each_with_object without
the need to return the object at the end.

However, we can't replace with each_with_object if the accumulator
parameter is assigned to within the block.

```ruby
# bad
[1, 2].inject({}) { |a, e| a[e] = e; a }

# good
[1, 2].each_with_object({}) { |e, a| a[e] = e }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only a block with exactly two required positional parameters and no other
parameter kind (optional/rest/post/keyword/block) is recognized as a
candidate, matching the given corpus; upstream's node pattern is looser
(`(args $_ $_)` matches any two parameters of any kind) but no fixture or
spec case exercises that.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let method = call.name();
        let method = method.as_slice();
        if method != b"inject" && method != b"reduce" {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };

        // `(call _ {:inject :reduce} _)`: exactly one argument to inject/reduce.
        let Some(args) = call.arguments() else { return };
        let args = args.arguments();
        if args.len() != 1 {
            return;
        }
        let method_arg = args.first().expect("len checked above");
        if is_basic_literal(&method_arg) {
            return;
        }

        let Some(params) = block.parameters() else { return };
        match params.kind() {
            NodeKind::BlockParametersNode => {
                check_block(ctx, method, message_loc.span(), &block);
            }
            NodeKind::NumberedParametersNode => {
                check_numblock(ctx, method, message_loc.span(), &block);
            }
            _ => {}
        }
    }
}

/// rubocop-ast's `Node#basic_literal?`: `BASIC_LITERALS.include?(type)`,
/// where `BASIC_LITERALS` is `LITERALS - COMPOSITE_LITERALS` -- unlike
/// `Lint/EachWithObjectArgument`'s `immutable_literal?`
/// ([`crate::lint::each_with_object_argument`]), this also accepts a plain
/// (non-interpolated) string, and excludes `dsym` (an interpolated symbol
/// is composite, even though it is otherwise immutable).
fn is_basic_literal(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::SymbolNode
            | NodeKind::TrueNode
            | NodeKind::FalseNode
            | NodeKind::NilNode
            | NodeKind::ImaginaryNode
            | NodeKind::RationalNode
    )
}

/// The block's own body, narrowed to its statement list -- `None` for an
/// empty `{ }`/`do end` body (Prism's `body()` is `None` there; RuboCop's
/// `return unless body`).
fn statements<'pr>(block: &BlockNode<'pr>) -> Option<ruby_ast::node::NodeList<'pr>> {
    let body = block.body()?;
    Some(body.as_statements_node()?.body())
}

/// RuboCop's `each_with_object_block_candidate?` guards plus `on_block`'s
/// body: exactly two required block parameters, a body whose last statement
/// bare-returns the first parameter, and no reassignment of that
/// accumulator anywhere in the body.
fn check_block(ctx: &mut Context<'_>, method: &[u8], selector: Span, block: &BlockNode<'_>) {
    let Some(params) = block.parameters().and_then(|p| p.as_block_parameters_node()) else {
        return;
    };
    let Some(inner) = params.parameters() else { return };
    if !inner.optionals().is_empty()
        || inner.rest().is_some()
        || !inner.posts().is_empty()
        || !inner.keywords().is_empty()
        || inner.keyword_rest().is_some()
        || inner.block().is_some()
    {
        return;
    }
    let reqs = inner.requireds();
    if reqs.len() != 2 {
        return;
    }
    let mut reqs = reqs.iter();
    let first = reqs.next().expect("len checked above");
    let second = reqs.next().expect("len checked above");
    let Some(first_name) = first.as_required_parameter_node().map(|p| p.name()) else { return };
    let accumulator = first_name.as_slice();

    let Some(stmts) = statements(block) else { return };
    let Some(return_value) = stmts.last() else { return };
    let Some(return_lvar) = return_value.as_local_variable_read_node() else { return };
    if return_lvar.name().as_slice() != accumulator {
        return;
    }

    let body = block.body().expect("statements() returned Some");
    if assigns_to(&body, accumulator) {
        return;
    }

    let message = format!("Use `each_with_object` instead of `{}`.", method_name(method));
    let mut edits = vec![
        Edit::replace(selector, b"each_with_object".to_vec()),
        Edit::replace(first.span(), ctx.text(second.span()).to_vec()),
        Edit::replace(second.span(), ctx.text(first.span()).to_vec()),
    ];
    let return_span = return_value.span();
    if whole_line_occupied(ctx, return_span) {
        edits.push(Edit::delete(ctx.whole_lines(return_span)));
    } else {
        edits.push(Edit::delete(return_span));
    }
    ctx.report_with_fix(
        &EachWithObject::META,
        selector,
        message,
        Fix { applicability: Applicability::Safe, edits },
    );
}

/// RuboCop's `each_with_object_numblock_candidate?` guards plus
/// `on_numblock`'s body: exactly two numbered parameters (`_1`/`_2`) and a
/// body whose last statement bare-returns `_1`. Unlike the block form, there
/// is no accumulator-reassignment guard (upstream does not call
/// `accumulator_param_assigned_to?` from `on_numblock` either).
fn check_numblock(ctx: &mut Context<'_>, method: &[u8], selector: Span, block: &BlockNode<'_>) {
    let Some(numbered) = block.parameters().and_then(|p| p.as_numbered_parameters_node()) else {
        return;
    };
    if numbered.maximum() != 2 {
        return;
    }
    let Some(stmts) = statements(block) else { return };
    let Some(return_value) = stmts.last() else { return };
    let Some(return_lvar) = return_value.as_local_variable_read_node() else { return };
    if return_lvar.name().as_slice() != b"_1" {
        return;
    }

    let message = format!("Use `each_with_object` instead of `{}`.", method_name(method));
    let mut edits = vec![Edit::replace(selector, b"each_with_object".to_vec())];
    let body = block.body().expect("statements() returned Some");
    each_descendant(&body, &mut |n| {
        let Some(lvar) = n.as_local_variable_read_node() else { return };
        match lvar.name().as_slice() {
            b"_1" => edits.push(Edit::replace(n.span(), b"_2".to_vec())),
            b"_2" => edits.push(Edit::replace(n.span(), b"_1".to_vec())),
            _ => {}
        }
    });
    ctx.report_with_fix(
        &EachWithObject::META,
        selector,
        message,
        Fix { applicability: Applicability::Safe, edits },
    );
}

/// `%<method>s` in `MSG`: the plain method name, `inject` or `reduce`.
fn method_name(method: &[u8]) -> &str {
    std::str::from_utf8(method).expect("method name is `inject` or `reduce`")
}

/// RuboCop's `accumulator_param_assigned_to?`: whether any descendant of
/// `body` writes to a local variable named `name`. See the module doc for
/// why checking these five Prism node kinds by name reproduces upstream's
/// identity comparison against a nested plain-assignment descendant.
fn assigns_to(body: &Node<'_>, name: &[u8]) -> bool {
    let mut found = false;
    each_descendant(body, &mut |n| {
        if found {
            return;
        }
        let written = match n.kind() {
            NodeKind::LocalVariableWriteNode => n.as_local_variable_write_node().map(|w| w.name()),
            NodeKind::LocalVariableOperatorWriteNode => {
                n.as_local_variable_operator_write_node().map(|w| w.name())
            }
            NodeKind::LocalVariableAndWriteNode => {
                n.as_local_variable_and_write_node().map(|w| w.name())
            }
            NodeKind::LocalVariableOrWriteNode => {
                n.as_local_variable_or_write_node().map(|w| w.name())
            }
            NodeKind::LocalVariableTargetNode => {
                n.as_local_variable_target_node().map(|w| w.name())
            }
            _ => None,
        };
        if written.is_some_and(|w| w.as_slice() == name) {
            found = true;
        }
    });
    found
}

/// RuboCop's `return_value_occupies_whole_line?`: `span`'s whole line(s),
/// stripped of surrounding whitespace, is exactly `span`'s own text --
/// nothing else shares that line.
fn whole_line_occupied(ctx: &Context<'_>, span: Span) -> bool {
    let whole = ctx.whole_lines(span);
    trim_ruby_whitespace(ctx.text(whole)) == ctx.text(span)
}

/// Ruby's `String#strip`, applied to raw bytes: trims ASCII whitespace
/// (space, tab, `\n`, `\r`, vertical tab, form feed) from both ends.
fn trim_ruby_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| !is_ruby_whitespace(b)).unwrap_or(bytes.len());
    let end = bytes.iter().rposition(|&b| !is_ruby_whitespace(b)).map_or(start, |i| i + 1);
    &bytes[start..end]
}
