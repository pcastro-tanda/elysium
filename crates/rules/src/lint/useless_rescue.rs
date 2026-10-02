//! `Lint/UselessRescue`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_rescue.rb`.
//!
//! # The whole multi-clause construct, not each `resbody`
//!
//! Upstream's `on_rescue(node)` fires once per whitequark `:rescue` node --
//! the *aggregate* node holding every `resbody` branch of one `begin ...
//! rescue ... rescue ... end` (or implicit `def`-body) construct as
//! children -- and inspects only `node.resbody_branches.last`. Prism has no
//! such aggregate: each `rescue` clause is its own [`NodeKind::RescueNode`],
//! chained forward through `subsequent()`. This rule therefore only acts on
//! the *head* of a chain (detected via `ctx.parent()` not itself being a
//! `RescueNode`, i.e. this clause is reached from a `BeginNode`, never from
//! another `RescueNode`'s `subsequent`), then walks `subsequent()` to the
//! last link itself -- reproducing `resbody_branches.last` exactly.
//!
//! # Finding the enclosing `ensure` without typed ancestor access
//!
//! Upstream's `use_exception_variable_in_ensure?` walks
//! `resbody_node.each_ancestor(:ensure).first` -- in whitequark, an `ensure`
//! clause's node *wraps* its `rescue` node as a child, so this always finds
//! the nearest textually-enclosing `ensure`, regardless of how many
//! `def`/`block`/`begin` layers sit in between. In Prism, `BeginNode`'s
//! `rescue_clause()` and `ensure_clause()` are sibling fields, not nested --
//! so this rule instead subscribes to [`NodeKind::BeginNode`] and, on
//! `enter`, pre-extracts every local-variable name read anywhere in that
//! `BeginNode`'s own `ensure_clause` body (if it has one) into
//! `ensure_locals`, pushed nearest-last and popped on `leave`; storing
//! names rather than the typed `StatementsNode` itself avoids tying this
//! struct's own lifetime to the parse tree.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::RescueNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Useless `rescue` detected.";

/// Checks for useless `rescue`s, which only reraise rescued exceptions.
#[derive(Debug, Clone)]
pub struct UselessRescue {
    /// See the module doc comment: local-variable names read anywhere
    /// within the nearest enclosing `ensure` clause's body, nearest last.
    ensure_locals: Vec<Vec<Vec<u8>>>,
}

impl Rule for UselessRescue {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessRescue",
        department: Department::Lint,
        summary: "Checks for useless `rescue`s.",
        explanation: "\
Checks for useless ``rescue``s, which only reraise rescued exceptions.

```ruby
# bad
def foo
  do_something
rescue
  raise
end

# bad
def foo
  do_something
rescue => e
  raise # or 'raise e', or 'raise $!', or 'raise $ERROR_INFO'
end

# good
def foo
  do_something
rescue
  do_cleanup
  raise
end

# bad (latest rescue)
def foo
  do_something
rescue ArgumentError
  # noop
rescue
  raise
end

# good (not the latest rescue)
def foo
  do_something
rescue ArgumentError
  raise
rescue
  # noop
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::BeginNode, NodeKind::RescueNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { ensure_locals: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::BeginNode { .. } => {
                let begin = node.as_begin_node().expect("kind matched");
                if let Some(ensure_clause) = begin.ensure_clause() {
                    let mut names = Vec::new();
                    if let Some(statements) = ensure_clause.statements() {
                        let statements_node = statements.as_node();
                        ruby_ast::each_descendant(&statements_node, &mut |descendant| {
                            if let Node::LocalVariableReadNode { .. } = descendant {
                                names.push(ctx.text(descendant.span()).to_vec());
                            }
                        });
                    }
                    self.ensure_locals.push(names);
                }
            }
            Node::RescueNode { .. } => {
                if ctx.parent().is_some_and(|p| p.kind == NodeKind::RescueNode) {
                    return;
                }
                let head = node.as_rescue_node().expect("kind matched");
                let mut last = head;
                while let Some(next) = last.subsequent() {
                    last = next;
                }
                if self.only_reraising(ctx, &last) {
                    let span = resbody_span(ctx, &last);
                    ctx.report(&Self::META, span, MSG);
                }
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if let Node::BeginNode { .. } = node {
            let begin = node.as_begin_node().expect("kind matched");
            if begin.ensure_clause().is_some() {
                self.ensure_locals.pop();
            }
        }
    }
}

impl UselessRescue {
    /// RuboCop's `only_reraising?`.
    fn only_reraising(&self, ctx: &Context<'_>, resbody: &RescueNode<'_>) -> bool {
        if self.use_exception_variable_in_ensure(ctx, resbody) {
            return false;
        }
        let Some(statements) = resbody.statements() else { return false };
        let body = statements.body();
        if body.len() != 1 {
            return false;
        }
        let Some(call) = body.first().and_then(|n| n.as_call_node()) else { return false };
        if call.receiver().is_some() || call.name().as_slice() != b"raise" {
            return false;
        }
        let Some(arguments) = call.arguments() else { return true };
        let args = arguments.arguments();
        if args.is_empty() {
            return true;
        }
        if args.len() > 1 {
            return false;
        }
        let exception_name = ctx.text(args.first().expect("len checked").span());
        exception_objects(ctx, resbody)
            .iter()
            .any(|candidate| candidate.as_slice() == exception_name)
    }

    /// RuboCop's `use_exception_variable_in_ensure?`.
    fn use_exception_variable_in_ensure(
        &self,
        ctx: &Context<'_>,
        resbody: &RescueNode<'_>,
    ) -> bool {
        let Some(reference) = resbody.reference() else { return false };
        let Some(names) = self.ensure_locals.last() else { return false };
        let exception_text = ctx.text(reference.span());
        names.iter().any(|name| name.as_slice() == exception_text)
    }
}

/// RuboCop's `exception_objects`.
fn exception_objects(ctx: &Context<'_>, resbody: &RescueNode<'_>) -> Vec<Vec<u8>> {
    let mut objects = vec![b"$!".to_vec(), b"$ERROR_INFO".to_vec()];
    if let Some(reference) = resbody.reference() {
        objects.push(ctx.text(reference.span()).to_vec());
    }
    objects
}

/// RuboCop's `offense_range`-equivalent for a `resbody`, reconstructed from
/// its own keyword/statements/reference/`=>`-variable/exception-list
/// children (same shape as `Lint/SuppressedException`'s own `resbody_span`;
/// copied privately per the porting kit rather than shared).
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

/// See [`resbody_span`]: scans forward from a body-less resbody's end, over
/// horizontal whitespace only, for an immediate `;` and extends the span
/// past it when found.
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
