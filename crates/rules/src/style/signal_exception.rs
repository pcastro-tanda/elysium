//! `Style/SignalException`, ported from RuboCop's
//! `lib/rubocop/cop/style/signal_exception.rb`.
//!
//! # `only_raise`/`only_fail`: no rescue-scope awareness
//!
//! Under the two non-`semantic` styles, upstream's `on_send` unconditionally
//! flags every `fail` (`only_raise`, unless a custom `fail` method is
//! defined anywhere in the file) or `raise` (`only_fail`) command/`Kernel`
//! call, with no regard for whether it sits inside a `begin`/`rescue`
//! at all -- confirmed by the `only_raise`-default fixture
//! `registers_an_offense_for_fail_in_begin_section`, which flags a bare
//! `fail` sitting in a `begin`'s main (pre-`rescue`) body exactly like it
//! would anywhere else. So these two styles need only a flat, generic
//! per-`CallNode` check.
//!
//! # `semantic`: reconstructing `on_rescue`'s scopes
//!
//! `semantic` style additionally special-cases two "scopes" upstream
//! reaches through whitequark's `:rescue` node (`RuboCop::AST::RescueNode`):
//! the body protected by a `begin`/`rescue` (`node.body`, checked for
//! `raise`) and each `resbody` branch's own body (`node.resbody_branches`,
//! checked for `fail`, plus every `raise` found there is `allow`ed --
//! marked so the generic `on_send` pass does not also flag it, since a
//! `raise` that rethrows inside a rescue handler is exactly what `semantic`
//! style wants).
//!
//! Both scopes are walked with `on_node(:send, node, :rescue)`: every
//! descendant `send` is visited, *except* none of the descendants of a
//! further nested `:rescue` node are -- that inner compound gets its own,
//! separate `on_rescue` call when the traversal reaches it, so re-entering
//! it here would double-count (see `is_not_confused_by_nested_begin_rescue`).
//! Prism has no single `:rescue` node standing for the whole
//! `begin ... rescue ... end`/`def ... rescue ... end` compound: it is a
//! [`NodeKind::BeginNode`] whose `rescue_clause` field, when present, holds
//! the first [`RescueNode`], linked to the next handler (if any) via
//! `RescueNode::subsequent` -- the same shape `duplicate_rescue_exception.rs`
//! reconstructs the branch list from. A bare `begin...end` with no
//! `rescue`/`ensure` is *also* a `BeginNode` (Prism does not distinguish
//! whitequark's separate `:kwbegin` node), but has `rescue_clause() ==
//! None`, so it is never itself a scope boundary here and [`each_scoped_call`]
//! correctly walks straight through it -- matching upstream, where only
//! `:rescue`-typed nodes stop the search, not `:kwbegin`/`:ensure` ones.
//! An implicit `def` body with a `rescue` is, per Prism, the exact same
//! `BeginNode` shape as an explicit `begin...end`, so subscribing to
//! [`NodeKind::BeginNode`] alone covers both without a separate `def` path.
//!
//! [`SignalException::handled`] is the `ignore_node`/`ignored_node?`
//! mechanism: [`each_scoped_call`] populates it (for both a reported `raise`
//! in the protected body and an `allow`ed `raise` in a handler body) while
//! visiting the owning `BeginNode`, strictly before the generic traversal
//! reaches those same `CallNode`s (parents are always entered first), so
//! the later, unconditional `CallNode` arm can simply check membership.
//!
//! # `command_or_kernel_call?`
//!
//! `node.command?(name)` (`rubocop-ast`'s `MethodDispatchNode`) is `!receiver
//! && method?(name)`; `kernel_call?` is the node-pattern
//! `(send (const {nil? cbase} :Kernel) %1 ...)`, a receiver that is a bare
//! or top-level-qualified (`::Kernel`) constant literally named `Kernel`.
//! [`command_or_kernel_call`] combines both, and `RESTRICT_ON_SEND`'s implicit
//! `on_send`-only (never `on_csend`) restriction is reproduced by rejecting
//! `CallNode::is_safe_navigation` up front in `enter`.
//!
//! # `custom_fail_defined?`
//!
//! `custom_fail_methods`'s node-search pattern `{(def :fail ...) (defs _
//! :fail ...)}` matches any `def`, instance or singleton, named `fail`,
//! regardless of a singleton def's receiver (the `_` wildcard, unlike a
//! `nil?` guard, accepts any value) -- so [`scan_custom_fail`] only checks
//! `DefNode::name`, ignoring `DefNode::receiver` entirely. Computed once in
//! `file_start`, mirroring upstream's own `@custom_fail_defined` memoization
//! (a full-file scan is otherwise wasted work per `fail` call).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;
use std::collections::HashSet;

/// RuboCop's `FAIL_MSG`.
const FAIL_MSG: &str = "Use `fail` instead of `raise` to signal exceptions.";
/// RuboCop's `RAISE_MSG`.
const RAISE_MSG: &str = "Use `raise` instead of `fail` to rethrow exceptions.";
/// `only_raise` style's fixed message.
const ALWAYS_RAISE_MSG: &str = "Always use `raise` to signal exceptions.";
/// `only_fail` style's fixed message.
const ALWAYS_FAIL_MSG: &str = "Always use `fail` to signal exceptions.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    OnlyRaise,
    OnlyFail,
    Semantic,
}

/// Checks for proper usage of fail and raise.
#[derive(Debug, Clone)]
pub struct SignalException {
    style: Style,
    /// See the module doc's "`custom_fail_defined?`" section.
    custom_fail_defined: bool,
    /// See the module doc's "`semantic`: reconstructing `on_rescue`'s
    /// scopes" section. Keyed by a flagged/allowed call's selector span
    /// start, unique enough within one file.
    handled: HashSet<u32>,
}

impl SignalException {
    /// RuboCop's `custom_fail_methods` node-search, run once per file.
    fn scan_custom_fail(node: &Node<'_>) -> bool {
        if let Some(def) = node.as_def_node() {
            if def.name().as_slice() == b"fail" {
                return true;
            }
        }
        let mut found = false;
        for_each_child(node, |child| {
            found |= Self::scan_custom_fail(child);
        });
        found
    }
}

/// `rubocop-ast`'s `(const {nil? cbase} :Kernel)`: a bare or
/// top-level-qualified (`::Kernel`) reference to the `Kernel` constant,
/// with no intervening namespace.
fn is_kernel_const(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Kernel")
        }
        NodeKind::ConstantPathNode => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|name| name.as_slice() == b"Kernel")
        }),
        _ => false,
    }
}

/// RuboCop's `command_or_kernel_call?`.
fn command_or_kernel_call(call: &CallNode<'_>, name: &[u8]) -> bool {
    if call.name().as_slice() != name {
        return false;
    }
    match call.receiver() {
        None => true,
        Some(receiver) => is_kernel_const(&receiver),
    }
}

/// A flagged/allowed call's selector span: RuboCop's `node.loc.selector`.
fn selector_span(call: &CallNode<'_>) -> Span {
    call.message_loc().map_or_else(|| call.as_node().span(), |loc| loc.span())
}

/// RuboCop's `add_offense(send_node.loc.selector, message: ...) { |corrector|
/// autocorrect(corrector, send_node) }`: `autocorrect` always replaces just
/// the selector with the other method's name (`node.loc.selector`'s
/// receiver, if any, is left untouched).
fn report(
    ctx: &mut Context<'_>,
    call: &CallNode<'_>,
    message: &'static str,
    replacement: &'static [u8],
) {
    let span = selector_span(call);
    let fix =
        Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, replacement)] };
    ctx.report_with_fix(&SignalException::META, span, message, fix);
}

/// Whether `node` is a further nested `begin`/`rescue` compound -- the
/// walk boundary described in the module doc.
fn is_rescue_boundary(node: &Node<'_>) -> bool {
    node.as_begin_node().is_some_and(|begin| begin.rescue_clause().is_some())
}

/// The protected-body scope: `check_scope(:raise, node.body)`. Every
/// `raise` command/`Kernel` call reachable without crossing a nested
/// `begin`/`rescue` is flagged (`Use fail instead of raise...`) and marked
/// [`SignalException::handled`].
fn check_body_scope(ctx: &mut Context<'_>, handled: &mut HashSet<u32>, root: &Node<'_>) {
    for_each_child(root, |child| {
        if let Some(call) = child.as_call_node() {
            if !call.is_safe_navigation() && command_or_kernel_call(&call, b"raise") {
                report(ctx, &call, FAIL_MSG, b"fail");
                handled.insert(selector_span(&call).start);
            }
        }
        if !is_rescue_boundary(child) {
            check_body_scope(ctx, handled, child);
        }
    });
}

/// A `resbody` branch's own scope: `check_scope(:fail, rescue_node)` plus
/// `allow(:raise, rescue_node)`. Every `fail` command/`Kernel` call is
/// flagged (`Use raise instead of fail...`); every `raise` one is left
/// unflagged (a rethrow), but still marked [`SignalException::handled`] so
/// the generic pass does not flag it either.
fn check_rescue_scope(ctx: &mut Context<'_>, handled: &mut HashSet<u32>, root: &Node<'_>) {
    for_each_child(root, |child| {
        if let Some(call) = child.as_call_node() {
            if !call.is_safe_navigation() {
                if command_or_kernel_call(&call, b"fail") {
                    report(ctx, &call, RAISE_MSG, b"raise");
                    handled.insert(selector_span(&call).start);
                } else if command_or_kernel_call(&call, b"raise") {
                    handled.insert(selector_span(&call).start);
                }
            }
        }
        if !is_rescue_boundary(child) {
            check_rescue_scope(ctx, handled, child);
        }
    });
}

impl Rule for SignalException {
    const META: RuleMeta = RuleMeta {
        name: "Style/SignalException",
        department: Department::Style,
        summary: "Checks for proper usage of fail and raise.",
        explanation: "\
Checks for uses of `fail` and `raise`.

* `only_raise` (default) - enforces the sole use of `raise`.
* `only_fail` - enforces the sole use of `fail`.
* `semantic` - uses `fail` to signal an exception, then uses `raise` to
  trigger an offense after it has been rescued.

```ruby
# EnforcedStyle: only_raise (default)
# bad
begin
  fail
rescue Exception
  # handle it
end

# good
begin
  raise
rescue Exception
  # handle it
end

# EnforcedStyle: semantic
# bad
begin
  raise
rescue Exception
  # handle it
end

def watch_out
  # Error thrown
rescue Exception
  fail
end

# good
begin
  fail
rescue Exception
  # handle it
end

def watch_out
  fail
rescue Exception
  raise 'Preferably with descriptive message'
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BeginNode, NodeKind::CallNode],
        config: &[linter::ConfigOption {
            name: "EnforcedStyle",
            default: linter::ConfigDefault::Str("only_raise"),
            allowed: &["only_raise", "only_fail", "semantic"],
            doc: "Whether `fail`, `raise`, or a `semantic` split between them is enforced.",
        }],
        blind_spots: "\
`custom_fail_defined?` only widens the `only_raise` exemption; it is not
consulted for `only_fail`/`semantic`, matching upstream.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "only_fail" => Style::OnlyFail,
            "semantic" => Style::Semantic,
            _ => Style::OnlyRaise,
        };
        Ok(Self { style, custom_fail_defined: false, handled: HashSet::new() })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.handled.clear();
        self.custom_fail_defined = Self::scan_custom_fail(&ctx.parsed().root());
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::BeginNode => {
                if self.style != Style::Semantic {
                    return;
                }
                let begin = node.as_begin_node().expect("kind matched");
                let Some(rescue) = begin.rescue_clause() else { return };
                if let Some(stmts) = begin.statements() {
                    check_body_scope(ctx, &mut self.handled, &stmts.as_node());
                }
                let mut current = Some(rescue);
                while let Some(r) = current {
                    if let Some(stmts) = r.statements() {
                        check_rescue_scope(ctx, &mut self.handled, &stmts.as_node());
                    }
                    current = r.subsequent();
                }
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if call.is_safe_navigation() {
                    return;
                }
                match self.style {
                    Style::OnlyRaise => {
                        if !self.custom_fail_defined && command_or_kernel_call(&call, b"fail") {
                            report(ctx, &call, ALWAYS_RAISE_MSG, b"raise");
                        }
                    }
                    Style::OnlyFail => {
                        if command_or_kernel_call(&call, b"raise") {
                            report(ctx, &call, ALWAYS_FAIL_MSG, b"fail");
                        }
                    }
                    Style::Semantic => {
                        if self.handled.remove(&selector_span(&call).start) {
                            return;
                        }
                        if command_or_kernel_call(&call, b"raise") {
                            report(ctx, &call, FAIL_MSG, b"fail");
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
