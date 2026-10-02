//! `Lint/UnreachablePatternBranch`, ported from RuboCop's
//! `lib/rubocop/cop/lint/unreachable_pattern_branch.rb`.
//!
//! Upstream reads a guard clause (`in pattern if cond`) off a dedicated
//! whitequark `in_pattern` child (`node.children[1]`), separate from the
//! pattern itself. Prism has no such field on [`NodeKind::InNode`]
//! (`pattern`, `statements`, `in_loc`, `then_loc` only) -- a guard instead
//! wraps the real pattern in an [`NodeKind::IfNode`]/[`NodeKind::UnlessNode`]
//! whose own `statements` holds the single real pattern (verified against a
//! real Prism parse: `in Integer if cond` reaches `InNode::pattern()` as an
//! `IfNode` with `predicate` the guard and its one-statement `statements`
//! the real `ConstantReadNode`). So a guarded branch is detected by matching
//! `InNode::pattern()`'s own kind against `IfNode`/`UnlessNode` directly,
//! and -- exactly like upstream's `guard.nil?` check -- the branch is never
//! treated as a catch-all when guarded, regardless of what the wrapped
//! pattern underneath would otherwise be.
//!
//! Upstream's `catch_all_pattern?` recurses through `:begin` (whitequark's
//! node for a parenthesized pattern, `(pattern)`) by unwrapping
//! `pattern.children[0]`; Prism's equivalent is
//! [`NodeKind::ParenthesesNode`] with an `Option<Node>` `body` (verified:
//! `in (_ | Integer) => y` nests a `ParenthesesNode` inside the
//! `CapturePatternNode`'s `value`).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CaseMatchNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str = "Unreachable `in` pattern branch detected.";

/// Upstream's `MSG_ELSE`.
const MSG_ELSE: &str = "Unreachable `else` branch detected.";

/// Checks for unreachable `in` pattern branches after an unconditional catch-all pattern.
#[derive(Debug, Clone)]
pub struct UnreachablePatternBranch;

impl Rule for UnreachablePatternBranch {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UnreachablePatternBranch",
        department: Department::Lint,
        summary:
            "Checks for unreachable `in` pattern branches after an unconditional catch-all pattern.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CaseMatchNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let case_match: CaseMatchNode<'_> = node.as_case_match_node().expect("kind matched");

        let mut catch_all_found = false;
        for cond in &case_match.conditions() {
            let in_node = cond.as_in_node().expect("case/in conditions are InNode");

            if catch_all_found {
                ctx.report(&Self::META, in_node.location().span(), MSG);
                continue;
            }

            let pattern = in_node.pattern();
            let guarded = matches!(pattern.kind(), NodeKind::IfNode | NodeKind::UnlessNode);
            if !guarded && is_catch_all_pattern(&pattern) {
                catch_all_found = true;
            }
        }

        if !catch_all_found {
            return;
        }
        if let Some(else_node) = case_match.else_clause() {
            ctx.report(&Self::META, else_node.else_keyword_loc().span(), MSG_ELSE);
        }
    }
}

/// Upstream's `catch_all_pattern?`.
fn is_catch_all_pattern(pattern: &Node<'_>) -> bool {
    match pattern.kind() {
        NodeKind::LocalVariableTargetNode => true,
        NodeKind::CapturePatternNode => {
            let capture = pattern.as_capture_pattern_node().expect("kind matched");
            is_catch_all_pattern(&capture.value())
        }
        NodeKind::ParenthesesNode => {
            let parens = pattern.as_parentheses_node().expect("kind matched");
            parens.body().is_some_and(|body| is_catch_all_pattern(&body))
        }
        NodeKind::AlternationPatternNode => {
            let alternation = pattern.as_alternation_pattern_node().expect("kind matched");
            is_catch_all_pattern(&alternation.left()) || is_catch_all_pattern(&alternation.right())
        }
        _ => false,
    }
}
