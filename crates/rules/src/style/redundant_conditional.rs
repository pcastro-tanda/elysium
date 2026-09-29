//! `Style/RedundantConditional`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_conditional.rb`.
//!
//! # Node shapes
//!
//! Whitequark's `node_matcher`s match the *raw*, unnormalized `if` s-expr:
//! `(if (send _ op _) true false)` for the plain form and
//! `(if (send _ op _) false true)` for the inverted one. Crucially, `unless`
//! parses to the same raw `:if` node type in whitequark, but with its
//! `if_branch`/`else_branch` *children already swapped at parse time*: for
//! `unless cond; a; else; b; end`, the raw children are `[cond, b, a]`, not
//! `[cond, a, b]` (verified against `Parser::CurrentRuby`). So the same two
//! node patterns fire on `unless` too, just matching the source's `then`
//! and `else` bodies in the opposite slots from a real `if`. Prism keeps
//! `UnlessNode` unswapped (its `statements()` is genuinely the source's
//! `then` body, `else_clause()` the source's `else` body), so
//! [`check_unless`] inverts the truth-table by hand to reproduce the same
//! observable matches -- see the table in that function.
//!
//! `elsif` is another `IfNode` reached through `IfNode::subsequent`,
//! distinguished from a genuine `if` only by `if_keyword_loc` reading
//! `"elsif"` (see `Lint/DuplicateElsifCondition`'s module doc for the same
//! shape); the traversal reaches it independently, so [`check_if`] applies
//! uniformly to both.
//!
//! # Offense range and autocorrection
//!
//! `add_offense(node, ...)` uses whitequark's own `node.source_range`: for
//! a head `if`/`unless`, that runs through the shared terminal `end`; for a
//! nested `elsif` link, it stops at the end of its own `else` branch's last
//! statement, deliberately excluding that (shared) `end` -- reconstructed
//! here by [`own_span`], the same shape as `Lint/LiteralAsCondition`'s
//! `if_own_span` (see that cop's module doc for the full proof against
//! `Prism.parse`).
//!
//! `replacement_condition` mirrors upstream exactly: a plain node's
//! replacement is just the (possibly negated) condition source; an
//! `elsif`'s replacement additionally reads `else\n<indent><condition>` (via
//! `Alignment#indentation`, ported as [`indentation_after`]), since
//! replacing only the `elsif` link's own range must still open a valid
//! `else` clause for the surrounding chain. `message` uses the same
//! replacement text, just with a leading `\n` prepended for the `elsif`
//! case (so the offense message itself contains a literal newline,
//! matching upstream byte-for-byte).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, ElseNode, IfNode, StatementsNode, UnlessNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG_PREFIX: &str = "This conditional expression can just be replaced by `";

/// Checks for redundant returning of true/false in conditionals.
#[derive(Debug, Clone)]
pub struct RedundantConditional {
    /// `Alignment#configured_indentation_width`, read from
    /// `Layout/IndentationWidth`'s `Width` (default `2`).
    indentation_width: i64,
}

impl Rule for RedundantConditional {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantConditional",
        department: Department::Style,
        summary: "Don't return true/false from a conditional.",
        explanation: "\
Checks for redundant returning of true/false in conditionals.

```ruby
# bad
x == y ? true : false

# bad
if x == y
  true
else
  false
end

# good
x == y

# bad
x == y ? false : true

# good
x != y
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let indentation_width = options
            .peer("Layout/IndentationWidth", "Width")
            .and_then(OptionValue::as_int)
            .unwrap_or(2);
        Ok(Self { indentation_width })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::IfNode => self.check_if(&node.as_if_node().expect("kind matched"), ctx),
            NodeKind::UnlessNode => {
                self.check_unless(&node.as_unless_node().expect("kind matched"), ctx);
            }
            _ => {}
        }
    }
}

impl RedundantConditional {
    /// RuboCop's `on_if` for a real `if`/`elsif`/ternary node: a match
    /// requires a single `true`/`false` literal in each branch and a
    /// comparison-operator condition, exactly like `redundant_condition?`/
    /// `redundant_condition_inverted?`. Un-negated when `then`/`else` are
    /// `true`/`false`, negated when they're `false`/`true`.
    fn check_if(&self, node: &IfNode<'_>, ctx: &mut Context<'_>) {
        let Some(else_node) = node.subsequent().and_then(|s| s.as_else_node()) else { return };
        let Some(then_bool) = single_bool(node.statements()) else { return };
        let Some(else_bool) = single_bool(else_node.statements()) else { return };
        let Some(inverted) = classify(then_bool, else_bool) else { return };
        if !is_comparison_call(&node.predicate()) {
            return;
        }

        let is_elsif = node.if_keyword_loc().is_some_and(|k| k.as_slice() == b"elsif");
        let span = own_span(node, &else_node);
        self.report(node.predicate().span(), inverted, is_elsif, span, ctx);
    }

    /// RuboCop's `on_if` as it fires on `unless` (same raw `:if` node type
    /// in whitequark, with branches already swapped at parse time -- see
    /// the module doc). Prism's `UnlessNode` keeps `statements()`/
    /// `else_clause()` unswapped, so the truth table here is the mirror of
    /// [`check_if`]'s: `then=true,else=false` (matching whitequark's
    /// already-swapped `(if cond false true)`, i.e. inverted) and
    /// `then=false,else=true` (whitequark's `(if cond true false)`, plain).
    fn check_unless(&self, node: &UnlessNode<'_>, ctx: &mut Context<'_>) {
        let Some(else_node) = node.else_clause() else { return };
        let Some(then_bool) = single_bool(node.statements()) else { return };
        let Some(else_bool) = single_bool(else_node.statements()) else { return };
        let Some(inverted) = classify(else_bool, then_bool) else { return };
        if !is_comparison_call(&node.predicate()) {
            return;
        }

        let start = node.keyword_loc().span().start;
        let end =
            node.end_keyword_loc().map_or_else(|| node.as_node().span().end, |e| e.span().end);
        self.report(node.predicate().span(), inverted, false, Span::new(start, end), ctx);
    }

    /// RuboCop's `message`/`replacement_condition`/`add_offense`, unified:
    /// builds the (possibly `elsif`-indented) replacement text, the message
    /// embedding it, and reports+fixes `span` with both.
    fn report(
        &self,
        condition_span: Span,
        inverted: bool,
        is_elsif: bool,
        span: Span,
        ctx: &mut Context<'_>,
    ) {
        let condition = ctx.text(condition_span);
        let mut expression = Vec::with_capacity(condition.len() + 2);
        if inverted {
            expression.extend_from_slice(b"!(");
            expression.extend_from_slice(condition);
            expression.push(b')');
        } else {
            expression.extend_from_slice(condition);
        }

        let replacement = if is_elsif {
            let indent = indentation_after(ctx, span.start, self.indentation_width);
            let mut out = Vec::with_capacity(expression.len() + indent.len() + 6);
            out.extend_from_slice(b"else\n");
            out.extend_from_slice(&indent);
            out.extend_from_slice(&expression);
            out
        } else {
            expression
        };

        let mut message = String::with_capacity(MSG_PREFIX.len() + replacement.len() + 2);
        message.push_str(MSG_PREFIX);
        if is_elsif {
            message.push('\n');
        }
        message.push_str(&String::from_utf8_lossy(&replacement));
        message.push_str("`.");

        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// RuboCop's `Alignment#indentation`: `offset(node) + (' ' * width)`, where
/// `offset(node)` is `node.loc.column` spaces -- here, the byte offset
/// where `span` (an `elsif` link's own span) starts.
fn indentation_after(ctx: &Context<'_>, offset: u32, width: i64) -> Vec<u8> {
    let column = usize::try_from(ctx.line_col(offset).column).expect("column fits usize");
    let total = column + usize::try_from(width.max(0)).unwrap_or(0);
    vec![b' '; total]
}

/// RuboCop's `redundant_condition?`/`redundant_condition_inverted?`
/// collapsed into one: `None` when neither the plain nor inverted shape
/// matches (both branches the same literal), `Some(false)` for the plain
/// (`then=true,else=false`) shape, `Some(true)` for the inverted
/// (`then=false,else=true`) one.
const fn classify(then_bool: bool, else_bool: bool) -> Option<bool> {
    match (then_bool, else_bool) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        _ => None,
    }
}

/// A branch body consisting of exactly one `true`/`false` literal and
/// nothing else.
fn single_bool(statements: Option<StatementsNode<'_>>) -> Option<bool> {
    let statements = statements?;
    let body = statements.body();
    if body.len() != 1 {
        return None;
    }
    let only = body.iter().next()?;
    match only.kind() {
        NodeKind::TrueNode => Some(true),
        NodeKind::FalseNode => Some(false),
        _ => None,
    }
}

/// RuboCop's `#{COMPARISON_OPERATOR_MATCHER}` node-pattern:
/// `(send _ {:== :=== :!= :<= :>= :> :<} _)` -- a `send` (never `csend`)
/// call to a comparison operator with a receiver slot (any, including
/// `nil`) and exactly one argument.
fn is_comparison_call(node: &Node<'_>) -> bool {
    let Some(call) = as_plain_call(node) else { return false };
    if call.is_safe_navigation() || call.block().is_some() {
        return false;
    }
    if !matches!(call.name().as_slice(), b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<") {
        return false;
    }
    call.arguments().is_some_and(|args| args.arguments().len() == 1)
}

fn as_plain_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    node.as_call_node()
}

/// See the module doc: reconstructs whitequark's own `source_range` for a
/// matched `if`/`elsif`/ternary node -- a head (`if_keyword_loc` reading
/// `"if"`) or a ternary (`if_keyword_loc: None`) runs through its own real
/// end (the shared terminal `end` keyword, or -- for a ternary -- Prism's
/// own correct span end); an `elsif` link's own span stops at the end of
/// its terminal `else` branch's last statement, excluding that (shared)
/// `end`.
fn own_span(node: &IfNode<'_>, else_node: &ElseNode<'_>) -> Span {
    let is_elsif = node.if_keyword_loc().is_some_and(|k| k.as_slice() == b"elsif");
    let start =
        node.if_keyword_loc().map_or_else(|| node.as_node().span().start, |k| k.span().start);
    let end = if is_elsif {
        else_node
            .statements()
            .map_or_else(|| else_node.else_keyword_loc().span().end, |s| s.as_node().span().end)
    } else {
        node.end_keyword_loc().map_or_else(|| node.as_node().span().end, |e| e.span().end)
    };
    Span::new(start, end)
}
