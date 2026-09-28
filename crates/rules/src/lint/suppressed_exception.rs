//! `Lint/SuppressedException`, ported from RuboCop's
//! `lib/rubocop/cop/lint/suppressed_exception.rb`.
//!
//! Upstream's `on_resbody(node)` fires once per whitequark `resbody` node,
//! which covers *both* an explicit/implicit `rescue` clause and `expr
//! rescue nil`-style modifier rescues (whitequark parses the modifier form
//! to a `:rescue` node wrapping a `:resbody` child too, built through the
//! very same `rescue_body`/`rescue_body_map` parser-gem builder -- see
//! `duplicate_rescue_exception.rs`'s module doc for why other cops that
//! subscribe to the *outer* `:rescue` node need a `rescue_modifier?` guard
//! that has no Prism equivalent). Prism instead gives the modifier form its
//! own [`ruby_ast::node::RescueModifierNode`] kind, so this rule subscribes
//! to both [`NodeKind::RescueNode`] and [`NodeKind::RescueModifierNode`] and
//! runs the same three checks against each, translating whitequark's single
//! `node.body`/`nil_body?` shape into each kind's own fields.
//!
//! # Offense range extends into the body, unlike sibling rescue cops
//!
//! `rescue_body_map` (the parser gem builder) sets a resbody's
//! `loc.expression` to `keyword.join(compstmt || then || exc_var ||
//! exc_list || keyword)` -- unlike `Lint/RescueException`/
//! `Lint/ShadowedException`'s offense ranges (which the `resbody` there
//! always reaches with an empty-or-comment-only body, so `compstmt` is
//! always absent), this cop's `nil`-body fixtures exercise the `compstmt`
//! branch directly (`rescue\n  nil\nend` has a real `NilNode` statement).
//! [`resbody_span`] mirrors the whole four-way fallback rather than only
//! the `then`/`exc_var`/`exc_list` tail those cops' own span helpers
//! recompute. This matters less than it looks: the fixture harness
//! ([`ruby_ast`]'s test runner) truncates any multi-line offense's carets
//! to the first line's own length, and `rescue`/`nil` sit on separate lines
//! in every such fixture, so the extra reach is invisible there -- it only
//! becomes observable (and is exercised) by the modifier form, where
//! `keyword` and body share a line (`something rescue nil`, `def x = y
//! rescue nil`) and the offense visibly underlines through to the body.
//!
//! # `comment_between_rescue_and_end?` without typed ancestor access
//!
//! Upstream's `node.each_ancestor(:kwbegin, :any_def, :any_block).first`
//! walks up from the resbody to the nearest enclosing explicit
//! `begin...end` (`:kwbegin` -- a `BeginNode` *with* a `begin_keyword_loc`;
//! a `def`'s implicit rescue-bearing body is a keyword-less `BeginNode` and
//! is skipped, per the `whitequark -> Prism` trap doc), `def`/`defs`, or
//! block, then compares its own first line against that ancestor's end
//! line (`loc.end&.line`, or `loc.last_line` for an endless `def` with no
//! `end` keyword). [`Context::ancestors`] only exposes `NodeInfo`
//! (kind/span, no typed field access), so this rule instead subscribes to
//! [`NodeKind::BeginNode`], [`NodeKind::DefNode`] and [`NodeKind::BlockNode`]
//! directly and pushes/pops each qualifying one's own closing line onto
//! `ancestor_ends` on `enter`/`leave` (skipping keyword-less `BeginNode`s
//! exactly like the `:kwbegin` filter) -- reading `ancestor_ends.last()`
//! when a `rescue` is reached reproduces "nearest enclosing kwbegin/def/
//! block" without a separate upward walk. A node's own closing line is
//! simply the line its own span ends on: for a `BeginNode`/`DefNode` with
//! an explicit `end` keyword that is the `end` keyword's line (the node's
//! span always extends through it); for an endless `def` it is the same
//! line the node's span already ends on with no `end` keyword to reach,
//! i.e. exactly upstream's `loc.last_line` fallback.

//! # A body-less `resbody` terminated by `;` instead of `then`
//!
//! The parser gem's grammar accepts a newline, a `;`, or the literal
//! keyword `then` as a body-less resbody's terminator, and its builder
//! (`rescue_body_map`) folds whichever one appears into `loc.expression`
//! *only* when it is the `;` spelling (verified against a real `parser`
//! gem process: `rescue\nend` reports `loc.expression` as just `"rescue"`,
//! but `rescue ActiveRecord::Rollback; end` reports
//! `"rescue ActiveRecord::Rollback;"`, one past the `;` -- a bare newline
//! terminator is never folded in, only a literal `;` is). Prism's
//! `RescueNode::then_keyword_loc` is `None` for both spellings, so
//! [`resbody_span`] recovers the `;` case the only way available: once
//! every other anchor (`statements`/`then_keyword_loc`/`reference`/
//! `exceptions`) is exhausted and the clause is confirmed body-less, it
//! scans forward from there over horizontal whitespace only (never past a
//! newline, matching the newline case staying un-folded) for an immediate
//! `;` and extends the span past it when found ([`extend_through_semicolon`]).

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{RescueNode, StatementsNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Do not suppress exceptions.";

/// RuboCop's `offense_range`-equivalent for a `resbody`, reconstructed from
/// its own keyword/statements/reference/`=>`-variable/exception-list
/// children. See the module doc comment for why this (unlike sibling
/// rescue cops) must include the `statements` branch.
fn resbody_span(ctx: &Context<'_>, rescue_node: &RescueNode<'_>) -> Span {
    let start = rescue_node.keyword_loc().span().start;
    let end = if let Some(statements) = rescue_node.statements() {
        statements.location().span().end
    } else if let Some(then_keyword) = rescue_node.then_keyword_loc() {
        then_keyword.span().end
    } else if let Some(reference) = rescue_node.reference() {
        reference.span().end
    } else if let Some(last_exception) = rescue_node.exceptions().last() {
        last_exception.span().end
    } else {
        rescue_node.keyword_loc().span().end
    };
    let end = if rescue_node.statements().is_none()
        && rescue_node.then_keyword_loc().is_none()
        && rescue_node.reference().is_none()
    {
        extend_through_semicolon(ctx, end)
    } else {
        end
    };
    Span::new(start, end)
}

/// See the module doc comment ("A body-less `resbody` terminated by `;`
/// instead of `then`"): scans forward from a body-less resbody's end,
/// over horizontal whitespace only, for an immediate `;` and extends the
/// span past it when found.
fn extend_through_semicolon(ctx: &Context<'_>, end: u32) -> u32 {
    let line_col = ctx.line_col(end);
    let line_text = ctx.line_text(line_col.line);
    let col = line_col.column as usize;
    let Some(rest) = line_text.get(col..) else { return end };
    let ws = rest.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
    if rest.get(ws) == Some(&b';') {
        end + u32::try_from(ws + 1).expect("offset exceeds u32")
    } else {
        end
    }
}

/// RuboCop's `nil_body?`: the clause's body is a single `nil` literal, not
/// several statements (which whitequark would wrap in a non-`nil_type?`
/// implicit `begin`) and not merely present (`statements` holding several
/// entries) or absent.
fn is_nil_body(statements: Option<StatementsNode<'_>>) -> bool {
    let Some(statements) = statements else { return false };
    let body = statements.body();
    body.len() == 1 && body.first().is_some_and(|first| first.kind() == NodeKind::NilNode)
}

/// RuboCop's `comment_line?`: the entire line, ignoring leading whitespace,
/// is a comment.
fn is_comment_line(ctx: &Context<'_>, line: u32) -> bool {
    ctx.line_text(line).iter().find(|b| !b.is_ascii_whitespace()).is_some_and(|&b| b == b'#')
}

/// RuboCop's `comment_between_rescue_and_end?`. See the module doc comment
/// for why the ancestor's end line is tracked on `self` rather than found
/// via a typed ancestor walk.
fn comment_between_rescue_and_end(
    ancestor_ends: &[u32],
    ctx: &Context<'_>,
    keyword_span: Span,
) -> bool {
    let Some(&end_line) = ancestor_ends.last() else { return false };
    let start_line = ctx.line_col(keyword_span.start).line;
    (start_line..end_line).any(|line| is_comment_line(ctx, line))
}

/// Checks for `rescue` blocks with no body.
#[derive(Debug, Clone)]
pub struct SuppressedException {
    allow_comments: bool,
    allow_nil: bool,
    /// Closing line of every currently-open `kwbegin`/`def`/`defs`/block
    /// ancestor, nearest last. See the module doc comment.
    ancestor_ends: Vec<u32>,
}

impl SuppressedException {
    /// Upstream's `on_resbody` body, shared by both the `RescueNode` and
    /// `RescueModifierNode` shapes: `body_present` is whitequark's
    /// `node.body` truthiness (a `RescueModifierNode`'s `rescue_expression`
    /// is never absent, so it is always `true` there), and `nil_body` is
    /// `nil_body?(node)`.
    fn check(
        &self,
        ctx: &mut Context<'_>,
        keyword_span: Span,
        body_present: bool,
        nil_body: bool,
        span: Span,
    ) {
        if body_present && !nil_body {
            return;
        }
        if self.allow_comments
            && comment_between_rescue_and_end(&self.ancestor_ends, ctx, keyword_span)
        {
            return;
        }
        if self.allow_nil && nil_body {
            return;
        }
        ctx.report(&Self::META, span, MSG);
    }
}

impl Rule for SuppressedException {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SuppressedException",
        department: Department::Lint,
        summary: "Checks for `rescue` blocks with no body.",
        explanation: "\
Checks for `rescue` blocks with no body. Such empty `rescue` clauses \
silently swallow every exception raised in the guarded code, which almost \
always hides a bug rather than fixing one.

```ruby
# bad
def some_method
  do_something
rescue
end

# good
def some_method
  do_something
rescue
  handle_exception
end
```

A comment in the `rescue` body is allowed by default (`AllowComments:
true`) since it at least documents the decision to suppress the exception,
and rescuing to an explicit `nil` is allowed by default (`AllowNil: true`)
for the same reason.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::BeginNode,
            NodeKind::DefNode,
            NodeKind::BlockNode,
            NodeKind::RescueNode,
            NodeKind::RescueModifierNode,
        ],
        config: &[
            ConfigOption {
                name: "AllowComments",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow a comment in place of a body.",
            },
            ConfigOption {
                name: "AllowNil",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Allow rescuing to an explicit `nil` body.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            allow_comments: options.bool("AllowComments"),
            allow_nil: options.bool("AllowNil"),
            ancestor_ends: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::BeginNode => {
                let begin = node.as_begin_node().expect("kind matched");
                if begin.begin_keyword_loc().is_some() {
                    self.ancestor_ends.push(ctx.line_col(node.span().end).line);
                }
            }
            NodeKind::DefNode | NodeKind::BlockNode => {
                self.ancestor_ends.push(ctx.line_col(node.span().end).line);
            }
            NodeKind::RescueNode => {
                let rescue = node.as_rescue_node().expect("kind matched");
                let statements = rescue.statements();
                let nil_body = is_nil_body(statements);
                let keyword_span = rescue.keyword_loc().span();
                let span = resbody_span(ctx, &rescue);
                self.check(ctx, keyword_span, statements.is_some(), nil_body, span);
            }
            NodeKind::RescueModifierNode => {
                let modifier = node.as_rescue_modifier_node().expect("kind matched");
                let keyword_span = modifier.keyword_loc().span();
                let body = modifier.rescue_expression();
                let nil_body = body.kind() == NodeKind::NilNode;
                let span = Span::new(keyword_span.start, body.span().end);
                self.check(ctx, keyword_span, true, nil_body, span);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::BeginNode => {
                let begin = node.as_begin_node().expect("kind matched");
                if begin.begin_keyword_loc().is_some() {
                    self.ancestor_ends.pop();
                }
            }
            NodeKind::DefNode | NodeKind::BlockNode => {
                self.ancestor_ends.pop();
            }
            _ => {}
        }
    }
}
