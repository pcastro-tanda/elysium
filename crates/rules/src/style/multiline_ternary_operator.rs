//! `Style/MultilineTernaryOperator`, ported from RuboCop's
//! `lib/rubocop/cop/style/multiline_ternary_operator.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_IF`.
const MSG_IF: &str = "Avoid multi-line ternary operators, use `if` or `unless` instead.";
/// RuboCop's `MSG_SINGLE_LINE`.
const MSG_SINGLE_LINE: &str = "Avoid multi-line ternary operators, use single-line instead.";

/// RuboCop-AST's `MethodIdentifierPredicates#assignment_method?`
/// (`method_name.end_with?('=') && !comparison_method?`).
fn is_assignment_method(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=")
}

/// A ternary branch's single statement (rubocop-ast's `IfNode#if_branch`/
/// `#else_branch`: whitequark elides the wrapping `begin` for a single
/// statement, so upstream's `node_parts[1]`/`[2]` are the bare statement;
/// Prism always wraps in a `StatementsNode`, whose sole element -- ternary
/// syntax admits only one expression per branch -- is that same statement).
fn branch(statements: Option<StatementsNode<'_>>) -> Option<Node<'_>> {
    statements?.body().first()
}

/// Avoid multi-line ?: (the ternary operator); use if/unless instead.
#[derive(Debug, Clone, Default)]
pub struct MultilineTernaryOperator {
    /// Per currently-open `CallNode` ancestor, RuboCop's combined
    /// `node.send_type? && node.assignment_method?` (`use_assignment_method?`):
    /// `false` for a safe-navigation call (Prism's `is_safe_navigation`,
    /// whitequark's distinct `:csend` type, never `send_type?`) regardless of
    /// name. Pushed/popped in lockstep with the real traversal, so the
    /// innermost entry always matches whichever `CallNode` [`Context::ancestors`]
    /// reports as the ternary's immediate (or, through Prism's `ArgumentsNode`
    /// wrapper, its grandparent) ancestor.
    call_stack: Vec<bool>,
}

impl MultilineTernaryOperator {
    /// RuboCop's `enforce_single_line_ternary_operator?`:
    /// `SINGLE_LINE_TYPES.include?(node.parent&.type) &&
    /// !use_assignment_method?(node.parent)`. `SINGLE_LINE_TYPES` is
    /// `%i[return break next send csend]`; Prism unifies `send`/`csend` into
    /// one `CallNode` kind, and wraps a call's/`return`'s/`break`'s/`next`'s
    /// arguments in an `ArgumentsNode` the whitequark AST never has, so the
    /// node whitequark would call `node.parent` is reached by skipping over
    /// that wrapper when present.
    fn enforce_single_line(&self, ctx: &Context<'_>) -> bool {
        let ancestors = ctx.ancestors();
        let Some(immediate) = ancestors.last() else { return false };
        let effective_kind = if immediate.kind == NodeKind::ArgumentsNode {
            let Some(grandparent) = ancestors.len().checked_sub(2).and_then(|i| ancestors.get(i))
            else {
                return false;
            };
            grandparent.kind
        } else {
            immediate.kind
        };
        match effective_kind {
            NodeKind::ReturnNode | NodeKind::BreakNode | NodeKind::NextNode => true,
            NodeKind::CallNode => !self.call_stack.last().copied().unwrap_or(false),
            _ => false,
        }
    }

    /// The insertion point for comments hoisted out of the ternary's own
    /// span (RuboCop's `corrector.insert_before(parent, ...)`, `parent`
    /// being the same normalized ancestor [`Self::enforce_single_line`]
    /// reads `parent&.type` from).
    fn parent_span(ctx: &Context<'_>) -> Option<Span> {
        let ancestors = ctx.ancestors();
        let immediate = ancestors.last()?;
        if immediate.kind == NodeKind::ArgumentsNode {
            ancestors.get(ancestors.len().checked_sub(2)?).map(|a| a.span)
        } else {
            Some(immediate.span)
        }
    }

    /// RuboCop's `comments_in_condition`: every comment RuboCop-AST's
    /// `CommentsHelp#comments_in_range` would find strictly inside the
    /// node's own span, each followed by a newline, in source order.
    fn hoisted_comments(ctx: &Context<'_>, span: Span) -> Vec<u8> {
        let mut out = Vec::new();
        for comment in ctx.comments() {
            if span.start <= comment.span.start && comment.span.end <= span.end {
                out.extend_from_slice(ctx.text(comment.span));
                out.push(b'\n');
            }
        }
        out
    }

    /// RuboCop's `on_if`/`offense?`/`autocorrect`.
    fn check_if(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let if_node = node.as_if_node().expect("kind matched");
        // `node.ternary?` (`loc?(:question)`): a real `if`/`elsif` has an
        // `if_keyword_loc`; only a true ternary omits it.
        if if_node.if_keyword_loc().is_some() {
            return;
        }
        let span = node.span();
        // `node.multiline?`.
        if ctx.is_single_line(span) {
            return;
        }
        let Some(if_branch) = branch(if_node.statements()) else { return };
        let Some(else_node) = if_node.subsequent().and_then(|s| s.as_else_node()) else { return };
        let Some(else_branch) = branch(else_node.statements()) else { return };
        let condition = if_node.predicate();

        let condition_src = ctx.text(condition.span());
        let if_branch_src = ctx.text(if_branch.span());
        let else_branch_src = ctx.text(else_branch.span());

        let single_line = self.enforce_single_line(ctx);
        let mut replacement = Vec::new();
        if single_line {
            replacement.extend_from_slice(condition_src);
            replacement.extend_from_slice(b" ? ");
            replacement.extend_from_slice(if_branch_src);
            replacement.extend_from_slice(b" : ");
            replacement.extend_from_slice(else_branch_src);
        } else {
            replacement.extend_from_slice(b"if ");
            replacement.extend_from_slice(condition_src);
            replacement.extend_from_slice(b"\n  ");
            replacement.extend_from_slice(if_branch_src);
            replacement.extend_from_slice(b"\nelse\n  ");
            replacement.extend_from_slice(else_branch_src);
            replacement.extend_from_slice(b"\nend");
        }

        // `node.source != replacement(node)`: a method-call condition split
        // across a line break (only the line break itself, not the ternary)
        // reconstructs byte-for-byte identical to its own source.
        if ctx.text(span) == replacement.as_slice() {
            return;
        }

        let message = if single_line { MSG_SINGLE_LINE } else { MSG_IF };
        let mut edits = vec![Edit::replace(span, replacement)];
        let hoisted = Self::hoisted_comments(ctx, span);
        if !hoisted.is_empty() {
            if let Some(parent_span) = Self::parent_span(ctx) {
                edits.push(Edit::insert(parent_span.start, hoisted));
            }
        }
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

impl Rule for MultilineTernaryOperator {
    const META: RuleMeta = RuleMeta {
        name: "Style/MultilineTernaryOperator",
        department: Department::Style,
        summary: "Avoid multi-line ?: (the ternary operator); use if/unless instead.",
        explanation: "\
Checks for multi-line ternary op expressions.

NOTE: `return if ... else ... end` is syntax error. If `return` is used before
multiline ternary operator expression, it will be autocorrected to single-line
ternary operator. The same is true for `break`, `next`, and method call.

```ruby
# bad
a = cond ?
  b : c
a = cond ? b :
    c
a = cond ?
    b :
    c

return cond ?
       b :
       c

# good
a = cond ? b : c
a = if cond
  b
else
  c
end

return cond ? b : c
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                let eligible =
                    !call.is_safe_navigation() && is_assignment_method(call.name().as_slice());
                self.call_stack.push(eligible);
            }
            NodeKind::IfNode => self.check_if(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode {
            self.call_stack.pop();
        }
    }
}
