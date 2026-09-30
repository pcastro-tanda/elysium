//! `Layout/MultilineOperationIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/multiline_operation_indentation.rb` plus the
//! `MultilineExpressionIndentation` mixin (see
//! [`crate::layout::multiline_expression_indentation`]) and
//! `AlignmentCorrector`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::multiline_expression_indentation::{
    align_correct, argument_in_method_call, assignment_rhs, column, correct_indentation,
    first_argument_span, indentation, is_operator_method, kw_node_with_special_indentation,
    left_hand_side, not_for_this_cop, operation_description, part_of_assignment_rhs, ArgKind, Idx,
    Tree, Widths,
};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Align the right operand with the left one in keyword and assignment
    /// contexts.
    Aligned,
    /// Always indent the right operand one width past the expression.
    Indented,
}

/// Checks indentation of binary operations that span more than one line.
#[derive(Debug, Clone)]
pub struct MultilineOperationIndentation {
    style: Style,
    widths: Widths,
}

impl Rule for MultilineOperationIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/MultilineOperationIndentation",
        department: Department::Layout,
        summary: "Checks indentation of binary operations that span more than one line.",
        explanation: "\
Checks the indentation of the right hand side operand in binary operations
that span more than one line.

The `aligned` style checks that operators are aligned if they are part of
an `if` or `while` condition, an explicit `return` statement, etc. In other
contexts, the second operand should be indented regardless of enforced
style.

In both styles, operators should be aligned when an assignment begins on
the next line.

```ruby
# EnforcedStyle: aligned (default)

# bad
if a +
    b
  something &&
  something_else
end

# good
if a +
   b
  something &&
    something_else
end
```

```ruby
# EnforcedStyle: indented

# bad
if a +
   b
  something &&
  something_else
end

# good
if a +
    b
  something &&
    something_else
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("aligned"),
                allowed: &["aligned", "indented"],
                doc: "Aligns the operands in keyword/assignment contexts \
                      (`aligned`) or always indents the right operand one \
                      width (`indented`).",
            },
            ConfigOption {
                name: "IndentationWidth",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "Overrides `Layout/IndentationWidth`'s configured width. \
                      Only accepted when `EnforcedStyle` is `indented`.",
            },
        ],
        blind_spots: "\
Autocorrection shifts the offending range exactly as upstream's
`AlignmentCorrector` does when handed a bare range: without the heredoc
taboo ranges it computes for a node, since upstream passes the offense
range, not the node.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "indented" => Style::Indented,
            _ => Style::Aligned,
        };
        if style == Style::Aligned
            && options.get("IndentationWidth").and_then(OptionValue::as_int).is_some()
        {
            return Err(options.error(
                "IndentationWidth",
                "The `Layout/MultilineOperationIndentation` cop only accepts an \
                 `IndentationWidth` configuration parameter when `EnforcedStyle` is \
                 `indented`.",
            ));
        }
        Ok(Self { style, widths: Widths::from_options(options) })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let tree = Tree::build(&root);
        for i in tree.indices() {
            match tree.kind(i) {
                NodeKind::AndNode | NodeKind::OrNode => self.check_and_or(&tree, ctx, i),
                NodeKind::CallNode => self.on_send(&tree, ctx, i),
                _ => {}
            }
        }
    }
}

impl MultilineOperationIndentation {
    /// The mixin's `on_send`/`on_csend`.
    fn on_send(&self, tree: &Tree<'_>, ctx: &mut Context<'_>, node: Idx) {
        let Some(call) = tree.call(node) else { return };
        let Some(receiver) = tree.receiver(node) else { return };
        if call.name().as_slice() == b"[]" {
            return;
        }
        // `relevant_node?`
        if tree.is_send(node) && is_unary_operation(tree, node) {
            return;
        }
        if tree.dot(node).is_some() {
            return;
        }
        let lhs = tree.span(left_hand_side(tree, receiver));
        let Some(rhs) = first_argument_span(tree, node) else { return };
        self.check(tree, ctx, node, lhs, rhs);
    }

    /// `check_and_or`.
    fn check_and_or(&self, tree: &Tree<'_>, ctx: &mut Context<'_>, node: Idx) {
        let prism = tree.node(node);
        let (left, right) = match prism {
            Node::AndNode { .. } => {
                let Some(and) = prism.as_and_node() else { return };
                (and.left().span(), and.right().span())
            }
            Node::OrNode { .. } => {
                let Some(or) = prism.as_or_node() else { return };
                (or.left().span(), or.right().span())
            }
            _ => return,
        };
        self.check(tree, ctx, node, left, right);
    }

    /// The mixin's `check` plus `offending_range`.
    fn check(&self, tree: &Tree<'_>, ctx: &mut Context<'_>, node: Idx, lhs: Span, rhs: Span) {
        if !ctx.begins_its_line(rhs) {
            return;
        }
        if not_for_this_cop(tree, ctx, node) {
            return;
        }
        let correct_column = if self.should_align(tree, ctx, node, rhs) {
            column(ctx, tree.span(node).start)
        } else {
            indentation(ctx, lhs) + correct_indentation(tree, node, self.widths)
        };
        let column_delta = correct_column - column(ctx, rhs.start);
        if column_delta == 0 {
            return;
        }
        let message = self.message(tree, ctx, node, lhs, rhs);
        let edits = align_correct(ctx, rhs, column_delta, &[]);
        if edits.is_empty() {
            ctx.report(&Self::META, rhs, message);
        } else {
            let fix = Fix { applicability: Applicability::Safe, edits };
            ctx.report_with_fix(&Self::META, rhs, message, fix);
        }
    }

    /// `should_align?`.
    fn should_align(&self, tree: &Tree<'_>, ctx: &Context<'_>, node: Idx, rhs: Span) -> bool {
        let assignment_node = part_of_assignment_rhs(tree, node, rhs);
        if let Some(assignment) = assignment_node {
            if assignment_rhs(tree, assignment).is_some_and(|rhs| ctx.begins_its_line(rhs)) {
                return true;
            }
        }
        if self.style != Style::Aligned {
            return false;
        }
        if kw_node_with_special_indentation(tree, node).is_some() || assignment_node.is_some() {
            return true;
        }
        argument_in_method_call(tree, ctx, node, ArgKind::WithOrWithoutParentheses)
            .is_some_and(|call| !def_modifier(tree, call))
    }

    /// `message`.
    fn message(
        &self,
        tree: &Tree<'_>,
        ctx: &Context<'_>,
        node: Idx,
        lhs: Span,
        rhs: Span,
    ) -> String {
        let what = operation_description(tree, ctx, node, rhs);
        if self.should_align(tree, ctx, node, rhs) {
            format!("Align the operands of {what} spanning multiple lines.")
        } else {
            let used = column(ctx, rhs.start) - indentation(ctx, lhs);
            let expected = correct_indentation(tree, node, self.widths);
            format!(
                "Use {expected} (not {used}) spaces for indenting {what} spanning multiple lines."
            )
        }
    }
}

/// RuboCop-AST's `unary_operation?`.
fn is_unary_operation(tree: &Tree<'_>, node: Idx) -> bool {
    let Some(selector) = tree.selector(node) else { return false };
    is_operator_method(tree, node) && selector.start == tree.span(node).start
}

/// RuboCop-AST's `def_modifier?`: `private def foo; end` and friends.
fn def_modifier(tree: &Tree<'_>, node: Idx) -> bool {
    let mut current = node;
    loop {
        let Some(arg) = first_argument_index(tree, current) else { return false };
        if matches!(tree.kind(arg), NodeKind::DefNode) {
            return true;
        }
        if !tree.is_call(arg) {
            return false;
        }
        current = arg;
    }
}

/// The tree index of a call's first whitequark argument.
fn first_argument_index(tree: &Tree<'_>, node: Idx) -> Option<Idx> {
    let first = first_argument_span(tree, node)?;
    tree.children(node).find(|&child| tree.node(child).span() == first)
}
