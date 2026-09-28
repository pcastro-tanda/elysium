//! `Lint/SafeNavigationChain`, ported from RuboCop's
//! `lib/rubocop/cop/lint/safe_navigation_chain.rb`.
//!
//! Prism collapses whitequark's `(send (any_block (csend ...) ...) ...)`
//! variant (an ordinary call chained after a safe-navigation call that
//! itself has a block) into the same shape as the plain
//! `(send (csend ...) ...)` case: the block lives on the receiver
//! `CallNode` itself (`block: Option<Node>`), so checking
//! `receiver.as_call_node().is_some_and(CallNode::is_safe_navigation)`
//! handles both without a separate branch.
//!
//! `parent.lhs.receiver != parent.rhs.receiver` (RuboCop's
//! `require_safe_navigation?`) is approximated by comparing the receivers'
//! source text; `AndNode` is visited before its children, so its `left`
//! and `right` receivers are recorded in `and_rhs_receiver_matches` keyed
//! by the `right` operand's span before that operand (our candidate
//! offense node) is visited.

use std::collections::HashMap;

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::StatementsNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not chain ordinary method call after safe navigation operator.";

/// `NilMethods#nil_methods`: `nil.methods` plus `other_stdlib_methods`
/// (`:to_d`, from `require "bigdecimal/util"`). Real-world Rails apps run
/// RuboCop with `rubocop-rails`/the app's own plugins loaded, which pull in
/// `ActiveSupport`'s `Object` core extensions, adding `blank?`/`present?`/
/// `try`/`in?`/`presence` (among others) to `NilClass`; both the upstream
/// spec's `accepts` fixtures and the discourse/mastodon corpora rely on
/// that, so they're included here too. `AllowedMethods` is empty since this
/// cop declares no `AllowedMethods` config in `default.yml`.
const NIL_METHODS: &[&[u8]] = &[
    b"!",
    b"!=",
    b"!~",
    b"&",
    b"<=>",
    b"==",
    b"===",
    b"=~",
    b"^",
    b"__id__",
    b"__send__",
    b"class",
    b"clone",
    b"define_singleton_method",
    b"display",
    b"dup",
    b"enum_for",
    b"eql?",
    b"equal?",
    b"extend",
    b"freeze",
    b"frozen?",
    b"hash",
    b"inspect",
    b"instance_eval",
    b"instance_exec",
    b"instance_of?",
    b"instance_variable_defined?",
    b"instance_variable_get",
    b"instance_variable_set",
    b"instance_variables",
    b"is_a?",
    b"itself",
    b"kind_of?",
    b"method",
    b"methods",
    b"nil?",
    b"object_id",
    b"private_methods",
    b"protected_methods",
    b"public_method",
    b"public_methods",
    b"public_send",
    b"rationalize",
    b"remove_instance_variable",
    b"respond_to?",
    b"send",
    b"singleton_class",
    b"singleton_method",
    b"singleton_methods",
    b"tap",
    b"then",
    b"to_a",
    b"to_c",
    b"to_d",
    b"to_enum",
    b"to_f",
    b"to_h",
    b"to_i",
    b"to_r",
    b"to_s",
    b"yield_self",
    b"|",
    b"blank?",
    b"present?",
    b"try",
    b"in?",
    b"presence",
];

/// `PLUS_MINUS_METHODS`: unary `+@`/`-@`.
const PLUS_MINUS_METHODS: &[&[u8]] = &[b"+@", b"-@"];

/// Do not chain ordinary method call after safe navigation operator.
#[derive(Debug, Clone, Default)]
pub struct SafeNavigationChain {
    /// `AndNode.right().span() -> parent.lhs.receiver == parent.rhs.receiver`,
    /// populated on entering the `AndNode` (visited before its children).
    and_rhs_receiver_matches: HashMap<Span, bool>,
    /// RuboCop's `ternary_safe_navigation?`: keyed by the span of the sole
    /// statement in an `IfNode`/`UnlessNode`'s truthy branch (normalized for
    /// `unless`), mapped to that conditional's predicate source text.
    /// Populated on entering the `IfNode`/`UnlessNode` (visited before its
    /// children), so it's available once the candidate offense (that sole
    /// statement) is visited.
    ternary_guard_predicate: HashMap<Span, Vec<u8>>,
}

impl Rule for SafeNavigationChain {
    const META: RuleMeta = RuleMeta {
        name: "Lint/SafeNavigationChain",
        department: Department::Lint,
        summary: "Do not chain ordinary method call after safe navigation operator.",
        explanation: "The safe navigation operator returns nil if the receiver is \
nil. If you chain an ordinary method call after a safe \
navigation operator, it raises NoMethodError. We should use a \
safe navigation operator after a safe navigation operator.\n\
This cop checks for the problem outlined above.",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::AndNode, NodeKind::IfNode, NodeKind::UnlessNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::AndNode => {
                let Some(and) = node.as_and_node() else { return };
                let matches =
                    call_receiver_text(&and.left(), ctx) == call_receiver_text(&and.right(), ctx);
                self.and_rhs_receiver_matches.insert(and.right().span(), matches);
            }
            NodeKind::IfNode => {
                let Some(if_node) = node.as_if_node() else { return };
                self.record_ternary_guard(if_node.predicate().span(), if_node.statements(), ctx);
            }
            NodeKind::UnlessNode => {
                let Some(unless_node) = node.as_unless_node() else { return };
                // `IfNode::if_branch`/`UnlessNode::if_branch` are normalized
                // in rubocop-ast: for `unless`, the branch that runs when the
                // predicate is truthy is the `else` clause, not `statements`.
                let branch = unless_node.else_clause().and_then(|e| e.statements());
                self.record_ternary_guard(unless_node.predicate().span(), branch, ctx);
            }
            NodeKind::CallNode => self.check_call(node, ctx),
            _ => {}
        }
    }
}

impl SafeNavigationChain {
    fn check_call(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        // Only plain `send` calls are candidates; a `csend` itself doesn't
        // match RuboCop's `(send (csend ...) ...)` pattern.
        if call.is_safe_navigation() {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        let Some(safe_nav) = receiver.as_call_node() else { return };
        if !safe_nav.is_safe_navigation() {
            return;
        }
        let method = call.name();
        let method = method.as_slice();
        if NIL_METHODS.contains(&method) || PLUS_MINUS_METHODS.contains(&method) {
            return;
        }
        if !self.require_safe_navigation(node, ctx) {
            return;
        }
        if self.ternary_safe_navigation(node, &safe_nav, ctx) {
            return;
        }
        let begin =
            call.call_operator_loc().map_or_else(|| receiver.span().end, |loc| loc.span().start);
        let span = Span::new(begin, node.span().end);
        ctx.report(&Self::META, span, MSG);
    }

    /// RuboCop's `require_safe_navigation?`: an offense is suppressed only
    /// when `node` is exactly the right-hand side of an `&&` whose left and
    /// right operands share the same receiver (e.g.
    /// `x&.foo&.bar && x&.foo.baz`, guarded by the earlier `x&.foo&.bar`).
    fn require_safe_navigation(&self, node: &Node<'_>, ctx: &Context<'_>) -> bool {
        match ctx.parent() {
            Some(p) if p.kind == NodeKind::AndNode => {
                match self.and_rhs_receiver_matches.get(&node.span()) {
                    Some(&matches) => !matches,
                    None => true,
                }
            }
            _ => true,
        }
    }

    /// RuboCop's `ternary_safe_navigation?`: an offense is suppressed when
    /// `node` (the candidate offense) is the sole statement of an
    /// `if`/`unless`/ternary's truthy branch, guarded by that same
    /// conditional's predicate (e.g. `x&.foo ? x&.foo.bar : baz`, where
    /// `x&.foo` being truthy already proves the chained `.bar` call safe).
    fn ternary_safe_navigation(
        &self,
        node: &Node<'_>,
        safe_nav: &ruby_ast::node::CallNode<'_>,
        ctx: &Context<'_>,
    ) -> bool {
        match self.ternary_guard_predicate.get(&node.span()) {
            Some(predicate_text) => {
                predicate_text.as_slice() == ctx.text(safe_nav.as_node().span())
            }
            None => false,
        }
    }

    /// Populates `ternary_guard_predicate` for an `IfNode`/`UnlessNode`'s
    /// truthy branch, keyed by that branch's sole statement's span (when it
    /// has exactly one), mapped to the conditional's predicate source text.
    fn record_ternary_guard(
        &mut self,
        predicate_span: Span,
        branch: Option<StatementsNode<'_>>,
        ctx: &Context<'_>,
    ) {
        let Some(stmts) = branch else { return };
        let body = stmts.body();
        if body.len() != 1 {
            return;
        }
        let Some(sole) = body.iter().next() else { return };
        self.ternary_guard_predicate.insert(sole.span(), ctx.text(predicate_span).to_vec());
    }
}

/// The receiver of `node` if it is a `CallNode` with one, as source text;
/// `None` for calls without a receiver (or non-`CallNode` operands, which
/// upstream's `Node#receiver` would raise on, not a case exercised here).
fn call_receiver_text<'a>(node: &Node<'_>, ctx: &Context<'a>) -> Option<&'a [u8]> {
    let receiver = node.as_call_node()?.receiver()?;
    Some(ctx.text(receiver.span()))
}
