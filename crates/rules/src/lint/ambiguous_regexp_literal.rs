//! `Lint/AmbiguousRegexpLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ambiguous_regexp_literal.rb`.
//!
//! # Prism shape
//!
//! Upstream drives entirely off `processed_source.diagnostics`: a parser
//! warning whose `reason` is `:ambiguous_regexp` (`:ambiguous_literal` on
//! Ruby <= 2.7, which Prism never parses, so that branch is dead here).
//! Prism reports the same situation as a parse warning whose message starts
//! with `` ambiguous `/` ``, at the same one-byte location (the opening
//! `/`) -- see [`ruby_ast::Parsed::warnings`].
//!
//! Upstream then walks up from the regexp node (`find_offense_node`) through
//! `send`/`csend` ancestors that are themselves a bare receiver-call chain
//! rooted at the regexp, stopping at the first ancestor whose own first
//! argument literally is the regexp, or at the top of the tree. Two
//! whitequark node-shape quirks that `find_offense_node`'s `send_type?`/
//! `receiver` checks rely on do not exist in Prism and are reproduced by
//! hand (see [`is_match2_like`]):
//!
//! - whitequark represents `<regexp> =~ <expr>` as a dedicated
//!   `match_with_lvasgn` node, not a `send` -- so it is never
//!   `first_argument_is_regexp?` (irrelevant, its own first argument is
//!   never the regexp) and, crucially, contributes no `.receiver` for the
//!   climb, which is what stops the climb at `assert /a/ =~ b` (the offense
//!   lands on the whole `=~` expression, not the outer `assert` call).
//!   Prism has no such special node: `<regexp> =~ <expr>` is a perfectly
//!   ordinary `CallNode` with `name() == "=~"` and `receiver()` set to the
//!   regexp, so it would otherwise incorrectly satisfy the climb condition
//!   and keep walking up to `assert`.
//! - `add_parentheses`'s `!node.respond_to?(:arguments)` fallback (wrap the
//!   whole node in bare parens, used for `match_with_lvasgn` since it has no
//!   `arguments` accessor) is reproduced as the same "wrap the offense
//!   node's own span" fix whenever the offense node is one of these `=~`
//!   calls, instead of the args-splicing fix real calls get.
//!
//! A `CallNode`'s span always extends through its own attached block
//! (unlike whitequark's separate `send`/`block` node pair), so
//! [`ruby_ast::ext::call_span_excluding_block`] recovers the whitequark
//! `send` node's span for both the "where do the arguments end" fix
//! computation and the (dead, in every fixture) empty-arguments case.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::call_span_excluding_block;
use ruby_ast::node::CallNode;
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

/// Upstream's `MSG`.
const MSG: &str = "Ambiguous regexp literal. Parenthesize the method arguments if it's surely a \
                    regexp literal, or add a whitespace to the right of the `/` if it should be \
                    a division.";

/// Checks for ambiguous regexp literals in the first argument of a method invocation without parentheses.
#[derive(Debug, Clone)]
pub struct AmbiguousRegexpLiteral;

impl Rule for AmbiguousRegexpLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Lint/AmbiguousRegexpLiteral",
        department: Department::Lint,
        summary: "Checks for ambiguous regexp literals in the first argument of a method invocation without parentheses.",
        explanation: "\
```ruby
# bad

# This is interpreted as a method invocation with a regexp literal,
# but it could possibly be `/` method invocations.
# (i.e. `do_something./(pattern)./(i)`)
do_something /pattern/i

# good

# With parentheses, there's no ambiguity.
do_something(/pattern/i)
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Upstream branches on `target_ruby_version >= 3.0` to also recognize the pre-3.0 `:ambiguous_literal`
diagnostic reason; Prism only ever parses as 3.3+, so that branch (and the `TargetRubyVersion`
option it depends on) is dead here and not read.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let targets: Vec<Span> = ctx
            .parsed()
            .warnings()
            .filter(|w| w.message.starts_with("ambiguous `/`"))
            .map(|w| w.span)
            .collect();
        if targets.is_empty() {
            return;
        }
        let root = ctx.parsed().root();
        let mut finder = Finder { targets, offenses: Vec::new(), stack: Vec::new() };
        walk(&root, &mut finder);
        for (warning_span, regexp_span, offense_node) in finder.offenses {
            let fix = build_fix(offense_node, regexp_span);
            ctx.report_with_fix(&Self::META, warning_span, MSG, fix);
        }
    }
}

/// Collects, for every target warning location, the offense node
/// `find_offense_node` resolves it to -- computed as soon as the matching
/// `RegularExpressionNode`/`InterpolatedRegularExpressionNode` is reached,
/// from the live ancestor stack (a full-tree traversal is needed since
/// [`Context::ancestors`] only tracks kind/span, not typed nodes with
/// `receiver`/`arguments` accessors).
struct Finder<'pr> {
    targets: Vec<Span>,
    offenses: Vec<(Span, Span, Node<'pr>)>,
    /// Ancestors of the node currently being entered, in root-to-leaf
    /// order, with Prism's `ArgumentsNode` wrappers omitted: whitequark has
    /// no such node (a `send`'s arguments are its own direct children), and
    /// `find_offense_node`'s `node.parent` climb assumes that flatter shape.
    stack: Vec<Node<'pr>>,
}

impl<'pr> Visitor<'pr> for Finder<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if is_regexp_kind(node.kind()) {
            if let Some(pos) = self.targets.iter().position(|t| t.start == node.span().start) {
                let warning_span = self.targets[pos];
                if let Some(offense_node) = find_offense_node(&self.stack, node.span()) {
                    self.offenses.push((warning_span, node.span(), offense_node));
                }
            }
        }
        if node.kind() != NodeKind::ArgumentsNode {
            self.stack.push(*node);
        }
    }

    fn leave(&mut self, node: &Node<'pr>) {
        if node.kind() != NodeKind::ArgumentsNode {
            self.stack.pop();
        }
    }
}

fn is_regexp_kind(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode)
}

/// Whitequark's `match_with_lvasgn`/`match2`: `<regexp> =~ <expr>`, only
/// when the regexp is literally the receiver. `<expr> =~ <regexp>` (regexp
/// on the right) stays an ordinary `send` upstream too, so is deliberately
/// not matched here.
fn is_match2_like(call: &CallNode<'_>, regexp: Span) -> bool {
    call.name().as_slice() == b"=~" && call.receiver().is_some_and(|r| r.span() == regexp)
}

/// Upstream's generic `Node#receiver`, minus whatever `receiver` a
/// [`is_match2_like`] call would otherwise expose (whitequark's
/// `match_with_lvasgn` has no `receiver` method at all).
fn sendlike_receiver<'pr>(node: &Node<'pr>, regexp: Span) -> Option<Node<'pr>> {
    let call = node.as_call_node()?;
    if is_match2_like(&call, regexp) {
        return None;
    }
    call.receiver()
}

/// `first_argument_is_regexp?`.
fn first_argument_is_regexp(node: &Node<'_>, regexp: Span) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if is_match2_like(&call, regexp) {
        return false;
    }
    call.arguments().and_then(|a| a.arguments().first()).is_some_and(|n| is_regexp_kind(n.kind()))
}

/// `method_chain_to_regexp_receiver?`, given `node = stack[idx]`.
fn method_chain_to_regexp_receiver(stack: &[Node<'_>], idx: usize, regexp: Span) -> bool {
    if idx == 0 {
        return false;
    }
    let parent = stack[idx - 1];
    let Some(parent_receiver) = sendlike_receiver(&parent, regexp) else { return false };
    if idx < 2 {
        return false;
    }
    sendlike_receiver(&parent_receiver, regexp).is_some_and(|r| r.span() == regexp)
}

/// `find_offense_node_by`/`find_offense_node`: `stack.last()` is the
/// regexp's immediate parent (upstream's `node.parent` seed); climbs while
/// the current node is a chained receiver call under a call ancestor, or
/// `method_chain_to_regexp_receiver?` holds, stopping as soon as a node's
/// own first argument is the regexp or there is no further ancestor.
fn find_offense_node<'pr>(stack: &[Node<'pr>], regexp: Span) -> Option<Node<'pr>> {
    let mut idx = stack.len().checked_sub(1)?;
    loop {
        let node = stack[idx];
        if first_argument_is_regexp(&node, regexp) || idx == 0 {
            return Some(node);
        }
        let parent = stack[idx - 1];
        let climbs = (parent.as_call_node().is_some()
            && sendlike_receiver(&node, regexp).is_some())
            || method_chain_to_regexp_receiver(stack, idx, regexp);
        if !climbs {
            return Some(node);
        }
        idx -= 1;
    }
}

/// `Util#add_parentheses`, restricted to the two shapes `find_offense_node`
/// can ever hand it: an ordinary call (splice parens around its arguments)
/// or an `=~` match call (wrap the whole expression, matching upstream's
/// `!node.respond_to?(:arguments)` fallback for `match_with_lvasgn`).
fn build_fix(node: Node<'_>, regexp: Span) -> Fix {
    if let Some(call) = node.as_call_node() {
        if !is_match2_like(&call, regexp) {
            return build_call_fix(&call);
        }
    }
    let span = node.span();
    Fix {
        applicability: Applicability::Safe,
        edits: vec![Edit::insert(span.start, b"(".to_vec()), Edit::insert(span.end, b")".to_vec())],
    }
}

/// `Util#add_parentheses`'s `send`-like branches: `args_begin`/`args_end`
/// splice parens around the call's own arguments (excluding any attached
/// block, which whitequark's separate `block` wrapper node never included
/// either); an argument-less call instead just gets `()` appended.
fn build_call_fix(call: &CallNode<'_>) -> Fix {
    let call_span = call_span_excluding_block(call);
    let has_arguments = call.arguments().is_some_and(|a| !a.arguments().is_empty());
    if !has_arguments {
        return Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::insert(call_span.end, b"()".to_vec())],
        };
    }
    let message_end = call.message_loc().map_or(call_span.start, |loc| loc.span().end);
    let args_begin = Span::new(message_end, message_end + 1);
    Fix {
        applicability: Applicability::Safe,
        edits: vec![
            Edit::replace(args_begin, b"(".to_vec()),
            Edit::insert(call_span.end, b")".to_vec()),
        ],
    }
}
