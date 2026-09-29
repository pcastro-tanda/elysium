//! `Style/NonNilCheck`, ported from RuboCop's
//! `lib/rubocop/cop/style/non_nil_check.rb`.
//!
//! # Node shapes
//!
//! Upstream's three node-pattern matchers, ported directly from their Prism
//! equivalents:
//!
//! - `not_equal_to_nil?` (`(send _ :!= nil)`): a [`NodeKind::CallNode`] named
//!   `!=` whose sole argument is a [`NodeKind::NilNode`] literal.
//! - `not_and_nil_check?` (`(send (send _ :nil?) :!)`): a `!` call whose
//!   receiver is itself a `nil?` call. `not x.nil?` and `!x.nil?` both parse
//!   to this same shape -- Prism spells `not` as an ordinary `!` `CallNode`
//!   too, with `message_loc` reading `"not"` instead of `"!"`.
//! - `unless_and_nil_check?` (`(if (send _ :nil?) ...)` gated on
//!   `parent.unless? && !parent.ternary?`): whitequark folds `if`/`unless`/
//!   ternary into one `:if` type, needing that extra gate; Prism gives
//!   `unless` (block or modifier form alike) its own dedicated
//!   [`NodeKind::UnlessNode`], distinct from the ternary/`if` shared
//!   [`NodeKind::IfNode`], so checking directly on entering an `UnlessNode`
//!   and inspecting its `predicate` reproduces the same match with no
//!   ternary/`if` case to exclude and no need to walk up from the child.
//!
//! None of the three checks the safe-navigation form (`RESTRICT_ON_SEND`
//! covers only `on_send`, never aliased to `on_csend`): a `&.nil?` predicate
//! or receiver is a [`ruby_prism::CallNode::is_safe_navigation`] `CallNode`,
//! guarded against explicitly wherever `nil?` is matched.
//!
//! # `ignore_node`/`on_def`
//!
//! Upstream's `on_def` (aliased `on_defs`) marks the last statement of a
//! predicate method's (`predicate_method?`: name ends in `?`) body as
//! ignored -- reported nowhere, corrected nowhere -- so a non-nil check used
//! as a method's own truthiness return value survives. Prism has one
//! `DefNode` for both instance and singleton (`def self.foo`/`def Foo.bar`)
//! definitions (unlike whitequark's separate `def`/`defs`), so one match arm
//! covers both; a method body is always wrapped in a `StatementsNode`
//! (whitequark elides that wrapper for a single statement), so upstream's
//! `body.begin_type? ? body.children.last : body` collapses to "the last
//! element of `body`'s `StatementsNode`, when it has one; otherwise `body`
//! itself" (the second arm only reachable for a `rescue`/`ensure` body,
//! which Prism gives a `BeginNode` instead). [`NonNilCheck::ignored_spans`]
//! is populated on entering each `DefNode`, before its body is walked, so
//! the span is already known by the time a candidate `CallNode` inside it is
//! reached.
//!
//! # `IncludeSemanticChanges`/`Style/NilComparison`
//!
//! `IncludeSemanticChanges` (default `false`) gates `!x.nil?`/`not x.nil?`
//! and `unless x.nil?` entirely: both change from "explicit but safe" to
//! "flagged and auto-corrected to bare truthiness", which can alter behavior
//! for a custom `!`/`nil?` override. Separately, whether `x != nil` itself
//! is even reachable depends on the peer `Style/NilComparison` cop: if it is
//! enabled with `EnforcedStyle: comparison` (preferring `x == nil` over
//! `x.nil?`) and `IncludeSemanticChanges` is off, converting `x != nil` to
//! `!x.nil?` would just have `NilComparison` flag it right back, so this
//! port also skips it, mirroring upstream's `nil_comparison_style` check.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG_FOR_REDUNDANCY`.
const MSG_FOR_REDUNDANCY: &str = "Explicit non-nil checks are usually redundant.";

/// rubocop-ast's `MethodIdentifierPredicates::OPERATOR_METHODS`, needed to
/// tell a genuine operator call (`a + b`) from an ordinary one for
/// [`is_operator_expression`]'s `send_type? && binary_operation?` arm.
const OPERATOR_METHODS: &[&[u8]] = &[
    b"|", b"^", b"&", b"<=>", b"==", b"===", b"=~", b">", b">=", b"<", b"<=", b"<<", b">>", b"+",
    b"-", b"*", b"/", b"%", b"**", b"~", b"+@", b"-@", b"!@", b"~@", b"[]", b"[]=", b"!", b"!=",
    b"!~", b"`",
];

/// Checks for redundant nil checks.
#[derive(Debug, Clone)]
pub struct NonNilCheck {
    /// `IncludeSemanticChanges`.
    include_semantic_changes: bool,
    /// Precomputed `!include_semantic_changes? && nil_comparison_style ==
    /// 'comparison'`: whether `Style/NilComparison`'s configuration forbids
    /// flagging `!=` at all. Independent of any node, so computed once.
    skip_comparison_offense: bool,
    /// RuboCop's `@ignored_nodes` (`IgnoredNode` mixin), restricted to what
    /// this cop ever ignores: the span of a predicate method's last
    /// statement. See the module doc.
    ignored_spans: Vec<Span>,
}

impl NonNilCheck {
    /// RuboCop's `on_def`/`on_defs`: records the span this `DefNode`'s body
    /// makes un-reportable, if it is a predicate method with a body. See the
    /// module doc for the `StatementsNode` collapse.
    fn record_ignored_span(&mut self, node: &Node<'_>) {
        let Some(def) = node.as_def_node() else { return };
        if !def.name().as_slice().ends_with(b"?") {
            return;
        }
        let Some(body) = def.body() else { return };
        let ignored = match body.as_statements_node() {
            Some(stmts) => stmts.body().last().map(|last| last.span()),
            None => Some(body.span()),
        };
        if let Some(span) = ignored {
            self.ignored_spans.push(span);
        }
    }

    /// RuboCop's `not_equal_to_nil?` plus `register_offense?`'s first
    /// branch: `x != nil`.
    fn check_not_equal_nil(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let span = call.as_node().span();
        if self.ignored_spans.contains(&span) || self.skip_comparison_offense {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let arguments = args.arguments();
        if arguments.len() != 1 {
            return;
        }
        let Some(argument) = arguments.first() else { return };
        if argument.kind() != NodeKind::NilNode {
            return;
        }
        let Some(receiver) = call.receiver() else { return };

        if self.include_semantic_changes {
            // RuboCop's `autocorrect_comparison`'s `include_semantic_changes?`
            // branch: `x != nil` -> `x`.
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, ctx.text(receiver.span()).to_vec())],
            };
            ctx.report_with_fix(&Self::META, span, MSG_FOR_REDUNDANCY, fix);
            return;
        }

        // RuboCop's `message`'s `node.method?(:!=) && !include_semantic_changes?`
        // branch: `x != nil` -> `!x.nil?`, with `MSG_FOR_REPLACEMENT`.
        let replacement = non_nil_check_replacement(&receiver, ctx);
        let current = String::from_utf8_lossy(ctx.text(span));
        let message = format!("Prefer `{replacement}` over `{current}`.");
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, replacement.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }

    /// RuboCop's `not_and_nil_check?`: `!x.nil?`/`not x.nil?`. Only reachable
    /// with `IncludeSemanticChanges: true`.
    fn check_not_nil_check(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        if !self.include_semantic_changes {
            return;
        }
        let span = call.as_node().span();
        if self.ignored_spans.contains(&span) {
            return;
        }
        let Some(inner) = call.receiver().and_then(|r| r.as_call_node()) else { return };
        if inner.name().as_slice() != b"nil?" || inner.is_safe_navigation() {
            return;
        }

        // RuboCop's `autocorrect_non_nil`: `!x.nil?` -> `x`, implicit
        // receiver (`!nil?`) -> `self`.
        let replacement: Vec<u8> = match inner.receiver() {
            Some(inner_receiver) => ctx.text(inner_receiver.span()).to_vec(),
            None => b"self".to_vec(),
        };
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, replacement)],
        };
        ctx.report_with_fix(&Self::META, span, MSG_FOR_REDUNDANCY, fix);
    }

    /// RuboCop's `unless_and_nil_check?`: `unless x.nil?` (block or modifier
    /// form). Only reachable with `IncludeSemanticChanges: true`. See the
    /// module doc for why this runs from the `UnlessNode` itself rather than
    /// from the `nil?` child.
    fn check_unless(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.include_semantic_changes {
            return;
        }
        let Some(unless_node) = node.as_unless_node() else { return };
        let predicate = unless_node.predicate();
        let Some(call) = predicate.as_call_node() else { return };
        if call.name().as_slice() != b"nil?" || call.is_safe_navigation() {
            return;
        }
        let span = predicate.span();
        if self.ignored_spans.contains(&span) {
            return;
        }

        // RuboCop's `autocorrect_unless_nil`: the `unless` keyword becomes
        // `if`, and the predicate `x.nil?` becomes `x` (`self` for an
        // implicit receiver, defensively -- upstream assumes one is always
        // present and reads `node.receiver.source` unconditionally).
        let receiver_source: Vec<u8> = match call.receiver() {
            Some(receiver) => ctx.text(receiver.span()).to_vec(),
            None => b"self".to_vec(),
        };
        let keyword_span = unless_node.keyword_loc().span();
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![
                Edit::replace(keyword_span, b"if".to_vec()),
                Edit::replace(span, receiver_source),
            ],
        };
        ctx.report_with_fix(&Self::META, span, MSG_FOR_REDUNDANCY, fix);
    }
}

/// RuboCop's `non_nil_check_replacement`: `!x.nil?`, parenthesizing `x` when
/// it is an operator expression that would otherwise bind looser than the
/// appended `.nil?`.
fn non_nil_check_replacement(receiver: &Node<'_>, ctx: &Context<'_>) -> String {
    let source = String::from_utf8_lossy(ctx.text(receiver.span()));
    if is_operator_expression(receiver) {
        format!("!({source}).nil?")
    } else {
        format!("!{source}.nil?")
    }
}

/// RuboCop's `operator_expression?`.
fn is_operator_expression(node: &Node<'_>) -> bool {
    match node.kind() {
        // `node.operator_keyword?`: rubocop-ast's own check matches `&&`/`||`
        // too, not only the `and`/`or` keyword spellings (see
        // `Style/RequireParentheses`'s `is_boolean_operator` for the same
        // observation).
        // `node.type?(:range, :iflipflop, :eflipflop)`: Prism has no
        // separate flip-flop node kind, using a `RangeNode` for both.
        NodeKind::AndNode | NodeKind::OrNode | NodeKind::RangeNode => true,
        // `node.if_type? && node.ternary?`.
        NodeKind::IfNode => node.as_if_node().is_some_and(|n| n.if_keyword_loc().is_none()),
        // `node.send_type? && node.binary_operation?`.
        NodeKind::CallNode => node.as_call_node().is_some_and(is_binary_operation),
        // `node.assignment?`.
        _ => node.kind().prism_name().ends_with("WriteNode"),
    }
}

/// RuboCop's `MethodDispatchNode#binary_operation?`.
fn is_binary_operation(call: CallNode<'_>) -> bool {
    if call.receiver().is_none() {
        return false;
    }
    if !OPERATOR_METHODS.contains(&call.name().as_slice()) {
        return false;
    }
    let Some(message_loc) = call.message_loc() else { return false };
    message_loc.span().start != call.as_node().span().start
}

impl Rule for NonNilCheck {
    const META: RuleMeta = RuleMeta {
        name: "Style/NonNilCheck",
        department: Department::Style,
        summary: "Checks for redundant nil checks.",
        explanation: "\
Checks for non-nil checks, which are usually redundant.

With `IncludeSemanticChanges` set to `false` by default, this cop
does not report offenses for `!x.nil?` and does no changes that might
change behavior.
Also `IncludeSemanticChanges` set to `false` with `EnforcedStyle: comparison` of
`Style/NilComparison` cop, this cop does not report offenses for `x != nil` and
does no changes to `!x.nil?` style.

With `IncludeSemanticChanges` set to `true`, this cop reports offenses
for `!x.nil?` and autocorrects that and `x != nil` to solely `x`, which
is *usually* OK, but might change behavior.

```ruby
# bad
if x != nil
end

# good
if x
end

# Non-nil checks are allowed if they are the final nodes of predicate.
# good
def signed_in?
  !current_user.nil?
end
```

With `IncludeSemanticChanges: false` (default):

```ruby
# good
if !x.nil?
end
```

With `IncludeSemanticChanges: true`:

```ruby
# bad
if !x.nil?
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::UnlessNode, NodeKind::DefNode],
        config: &[linter::ConfigOption {
            name: "IncludeSemanticChanges",
            default: linter::ConfigDefault::Bool(false),
            allowed: &[],
            doc: "When `true`, also flags (and auto-corrects, unsafely) `!x.nil?`/`not x.nil?` \
                  and `unless x.nil?`, reducing them to bare `x`/`if x`.",
        }],
        blind_spots: "\
Upstream's autocorrection for `unless x.nil?` reads `node.receiver.source` \
unconditionally and would raise on an implicit receiver (`unless nil?`); \
this port falls back to `self` instead of reproducing the crash.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let include_semantic_changes = options.bool("IncludeSemanticChanges");
        let nil_comparison_enabled = options
            .peer("Style/NilComparison", "Enabled")
            .and_then(OptionValue::as_bool)
            .unwrap_or(true);
        let nil_comparison_style = options
            .peer("Style/NilComparison", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            .unwrap_or("predicate");
        let skip_comparison_offense = !include_semantic_changes
            && nil_comparison_enabled
            && nil_comparison_style == "comparison";
        Ok(Self { include_semantic_changes, skip_comparison_offense, ignored_spans: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.ignored_spans.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::DefNode => self.record_ignored_span(node),
            NodeKind::UnlessNode => self.check_unless(node, ctx),
            NodeKind::CallNode => {
                let Some(call) = node.as_call_node() else { return };
                match call.name().as_slice() {
                    b"!=" => self.check_not_equal_nil(&call, ctx),
                    b"!" => self.check_not_nil_check(&call, ctx),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
