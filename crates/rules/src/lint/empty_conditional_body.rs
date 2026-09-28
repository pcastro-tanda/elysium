//! `Lint/EmptyConditionalBody`, ported from RuboCop's
//! `lib/rubocop/cop/lint/empty_conditional_body.rb` plus the
//! `CommentsHelp` mixin it includes.
//!
//! # Node shapes
//!
//! Whitequark's parser models `if`, `elsif`, and `unless` bodies as one
//! `:if`-type node, distinguished only by `node.keyword`; an `elsif` is
//! nested in the `else_branch` slot of its parent. Prism gives `unless` its
//! own [`UnlessNode`] (no `elsif` chain) and represents `elsif` as another
//! [`IfNode`] reached through [`IfNode::subsequent`], distinguished from a
//! genuine `if` only by `if_keyword_loc` reading `"elsif"` -- see
//! `Lint/DuplicateElsifCondition`'s module doc for the same shape.
//!
//! Handily, Prism gives *every* link of an `if`/`elsif` chain the same
//! shared `end_keyword_loc` (the chain's single closing `end`), unlike
//! whitequark's `Source::Map::Condition#end`, which is only ever set on the
//! node that actually owns the `kEND` token (the chain's head) -- every
//! nested `elsif` node has a `nil` `loc.end`. That whitequark quirk is what
//! upstream's `same_line?(node.loc.begin, node.loc.end)` skip relies on to
//! never fire for an `elsif` (an `elsif` can't have its own `end`), so here
//! that skip is only ever attempted for the chain head (`if`/`unless`,
//! never `elsif`, guarded on the keyword text).
//!
//! `node.loc.begin` (the `then`/`;` separator immediately after the
//! condition, when written explicitly) has no Prism equivalent at all --
//! Prism does not record `;` locations. But a same-physical-line
//! `if`/`unless`...`end` can only exist with some such separator, so
//! checking `same_line?(keyword, end_keyword)` directly is observationally
//! identical for the chain head.
//!
//! # Offense range
//!
//! Upstream's `offense_range` is `node.source_range.begin.join(node.loc.else.begin)`
//! when a subsequent `elsif`/`else` exists (spanning from this node's own
//! keyword up to, but excluding, the next link's keyword), else the node's
//! own `source_range`. For a genuine `if`/`unless` head with no subsequent,
//! that own range runs through the shared `end` keyword; for an `elsif`
//! with no subsequent (the chain's last link), whitequark's node never
//! extended that far in the first place (its own `source_range` stops at
//! its condition, since it never owned an `end` token to extend into) --
//! reproduced here by excluding the (Prism-shared) `end_keyword_loc` from
//! the range specifically for that `elsif` case.
//!
//! # Autocorrection
//!
//! `empty_if_branch?(node) && else_branch?(node)` (`remove_empty_branch`'s
//! condition for taking `branch_range` directly rather than
//! `deletion_range(branch_range(node))`) is true for every case reachable
//! here: `empty_if_branch?` is true whenever this node's own parent is not
//! itself an *unrelated* `if`/`elsif` chain whose own `then` branch is a
//! non-empty non-conditional statement -- a doubly-nested orphaned-if
//! shape no fixture exercises, and which `Context` could not check cheaply
//! anyway (only `NodeInfo { kind, span }` is exposed, not full parent
//! structure). In the reachable (`true`) case, `branch_range` becomes
//! `node.source_range.with(end_pos: node.loc.else.begin_pos)` -- the exact
//! same span `offense_range` computes for a node with a subsequent branch
//! -- so the deleted range here is simply the offense `range` itself: it
//! already swallows the trailing newline and the next line's own leading
//! indentation, both sitting between this node's end and the `else`
//! keyword's start.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ElseNode, IfNode, UnlessNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// What immediately follows this branch: the chain's terminal `end`,
/// another `elsif` link (its own keyword span), or a real `else` clause.
#[derive(Clone, Copy)]
enum Next<'pr> {
    None,
    Elsif(Span),
    Else(Span, ElseNode<'pr>),
}

/// Checks for the presence of `if`, `elsif` and `unless` branches without a body.
#[derive(Debug, Clone)]
pub struct EmptyConditionalBody {
    allow_comments: bool,
}

/// The pieces `check` needs from either an `IfNode` or `UnlessNode` link,
/// gathered here (rather than passed as separate arguments) to keep the
/// method under clippy's argument-count limit.
#[derive(Clone, Copy)]
struct Branch<'pr> {
    keyword: &'static str,
    is_elsif: bool,
    kw_span: Span,
    has_body: bool,
    own_end: Option<Span>,
    next: Next<'pr>,
    predicate: Node<'pr>,
}

impl EmptyConditionalBody {
    /// RuboCop's `on_if`, RuboCop's `CommentsHelp#contains_comments?`
    /// inlined, and `offense_range`/autocorrection, unified over `if`,
    /// `elsif`, and `unless` (see the module doc for how each maps onto
    /// this shared shape).
    fn check(&self, ctx: &mut Context<'_>, branch: Branch<'_>) {
        let Branch { keyword, is_elsif, kw_span, has_body, own_end, next, predicate } = branch;
        if has_body {
            return;
        }
        if !is_elsif {
            if let Some(end_span) = own_end {
                if ctx.same_line(kw_span, end_span) {
                    return;
                }
            }
        }

        let start_line = ctx.line_col(kw_span.start).line;
        let end_line = match next {
            Next::None => own_end.map_or(u32::MAX, |e| ctx.line_col(e.start).line),
            Next::Elsif(s) | Next::Else(s, _) => ctx.line_col(s.start).line,
        };
        if self.allow_comments
            && ctx.comments().iter().any(|c| c.line >= start_line && c.line < end_line)
        {
            return;
        }

        let range = match next {
            Next::None => {
                let end = own_end.map_or(kw_span.end, |e| if is_elsif { e.start } else { e.end });
                Span::new(kw_span.start, end)
            }
            Next::Elsif(s) | Next::Else(s, _) => Span::new(kw_span.start, s.start),
        };
        let message = format!("Avoid `{keyword}` branches without a body.");

        if let Next::Else(_, else_node) = next {
            if else_node.statements().is_some() {
                let inverse = match keyword {
                    "if" => "unless",
                    "unless" => "if",
                    _ => "",
                };
                let condition_text = ctx.text(predicate.span());
                let mut replacement = Vec::with_capacity(inverse.len() + 1 + condition_text.len());
                replacement.extend_from_slice(inverse.as_bytes());
                replacement.push(b' ');
                replacement.extend_from_slice(condition_text);

                // See the module doc: the deleted range is this same
                // `range` (upstream's `branch_range` in the reachable
                // `empty_if_branch?` case).
                let delete_span = range;

                ctx.report_with_fix(
                    &Self::META,
                    range,
                    message,
                    Fix {
                        applicability: Applicability::Safe,
                        edits: vec![
                            Edit::replace(else_node.else_keyword_loc().span(), replacement),
                            Edit::delete(delete_span),
                        ],
                    },
                );
                return;
            }
        }

        ctx.report(&Self::META, range, message);
    }

    /// RuboCop's `on_if`, fired for the chain's head `IfNode` and, via
    /// `subsequent`, once per nested `elsif` link (each visited
    /// independently as the traversal reaches it).
    fn check_if(&self, ctx: &mut Context<'_>, node: &IfNode<'_>) {
        let Some(kw_loc) = node.if_keyword_loc() else { return }; // ternary: no `if`/`elsif` keyword
        let is_elsif = kw_loc.as_slice() == b"elsif";
        let keyword = if is_elsif { "elsif" } else { "if" };
        let next = match node.subsequent() {
            None => Next::None,
            Some(sub) => {
                if let Some(else_node) = sub.as_else_node() {
                    Next::Else(else_node.else_keyword_loc().span(), else_node)
                } else {
                    let elsif = sub.as_if_node().expect("elsif chain link is an IfNode");
                    Next::Elsif(elsif.if_keyword_loc().expect("elsif has a keyword").span())
                }
            }
        };
        self.check(
            ctx,
            Branch {
                keyword,
                is_elsif,
                kw_span: kw_loc.span(),
                has_body: node.statements().is_some(),
                own_end: node.end_keyword_loc().map(|l| l.span()),
                next,
                predicate: node.predicate(),
            },
        );
    }

    /// RuboCop's `on_if` for an `unless` statement (whitequark models
    /// `unless` as the same `:if` node type; Prism gives it its own
    /// `UnlessNode`, with no `elsif`-style continuation).
    fn check_unless(&self, ctx: &mut Context<'_>, node: &UnlessNode<'_>) {
        let next = match node.else_clause() {
            None => Next::None,
            Some(else_node) => Next::Else(else_node.else_keyword_loc().span(), else_node),
        };
        self.check(
            ctx,
            Branch {
                keyword: "unless",
                is_elsif: false,
                kw_span: node.keyword_loc().span(),
                has_body: node.statements().is_some(),
                own_end: node.end_keyword_loc().map(|l| l.span()),
                next,
                predicate: node.predicate(),
            },
        );
    }
}

impl Rule for EmptyConditionalBody {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EmptyConditionalBody",
        department: Department::Lint,
        summary: "Checks for the presence of `if`, `elsif` and `unless` branches without a body.",
        explanation: "\
Checks for the presence of `if`, `elsif` and `unless` branches without a body.

NOTE: empty `else` branches are handled by `Style/EmptyElse`.

```ruby
# bad
if condition
end

# bad
unless condition
end

# bad
if condition
  do_something
elsif other_condition
end

# good
if condition
  do_something
end

# good
unless condition
  do_something
end

# good
if condition
  do_something
elsif other_condition
  nil
end

# good
if condition
  do_something
elsif other_condition
  do_something_else
end
```

AllowComments: true (default)

```ruby
# good
if condition
  do_something
elsif other_condition
  # noop
end
```

AllowComments: false

```ruby
# bad
if condition
  do_something
elsif other_condition
  # noop
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[ConfigOption {
            name: "AllowComments",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Whether a branch containing only comments counts as empty.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_comments: options.bool("AllowComments") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => {
                let n = node.as_if_node().expect("kind matched");
                self.check_if(ctx, &n);
            }
            Node::UnlessNode { .. } => {
                let n = node.as_unless_node().expect("kind matched");
                self.check_unless(ctx, &n);
            }
            _ => {}
        }
    }
}
