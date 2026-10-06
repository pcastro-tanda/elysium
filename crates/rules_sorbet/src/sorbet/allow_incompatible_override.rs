//! `Sorbet/AllowIncompatibleOverride`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signatures/allow_incompatible_override.rs`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

const MSG: &str = "Usage of `allow_incompatible` suggests a violation of the Liskov Substitution Principle. Instead, strive to write interfaces which respect subtyping principles and remove `allow_incompatible`";

/// Disallows using `.override(allow_incompatible: true)`.
#[derive(Debug, Clone)]
pub struct AllowIncompatibleOverride;

impl Rule for AllowIncompatibleOverride {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/AllowIncompatibleOverride",
        department: Department::Sorbet,
        summary: "Disallows using `.override(allow_incompatible: true)`.",
        explanation: "Disallows using `.override(allow_incompatible: true)`.\n\n```ruby\n# bad\nsig.override(allow_incompatible: true)\n\n# good\nsig.override\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };

        // on_send: `(send [!nil? #sig?] :override (hash <$pair ...>))`
        if !call.is_safe_navigation() && call.name().as_slice() == b"override" {
            if let Some(receiver) = call.receiver() {
                if contains_sig(&receiver) {
                    if let Some(span) = override_pair(&call) {
                        ctx.report(&Self::META, span, MSG);
                    }
                }
            }
        }

        // on_block / on_numblock / on_itblock
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        if !call_contains_sig(&call) {
            return;
        }
        let Some(body) = block.body() else { return };
        let Some(statements) = body.as_statements_node() else { return };
        let mut iter = statements.body().iter();
        let (Some(last), None) = (iter.next(), iter.next()) else { return };
        let Some(last) = last.as_call_node() else { return };
        if last.is_safe_navigation() || last.block().is_some_and(|b| b.as_block_node().is_some()) {
            return;
        }
        let mut receiver = last.receiver();
        while let Some(r) = receiver {
            let Some(rc) = r.as_call_node() else { break };
            if let Some(span) = override_pair(&rc) {
                ctx.report(&Self::META, span, MSG);
                break;
            }
            receiver = rc.receiver();
        }
    }
}

/// `sig?`: a `(send _ :sig ...)` anywhere inside `node` (itself included).
fn contains_sig(node: &Node<'_>) -> bool {
    let is_sig = |n: &Node<'_>| {
        n.as_call_node().is_some_and(|c| !c.is_safe_navigation() && c.name().as_slice() == b"sig")
    };
    let mut found = is_sig(node);
    each_descendant(node, &mut |n: &Node<'_>| found |= is_sig(n));
    found
}

/// `sig?(node.send_node)`: the call without its block.
fn call_contains_sig(call: &CallNode<'_>) -> bool {
    if !call.is_safe_navigation() && call.name().as_slice() == b"sig" {
        return true;
    }
    if call.receiver().is_some_and(|r| contains_sig(&r)) {
        return true;
    }
    call.arguments().is_some_and(|a| a.arguments().iter().any(|arg| contains_sig(&arg)))
}

/// `override?`: `(send _ :override (hash <$(pair (sym :allow_incompatible) true) ...>))`.
fn override_pair(call: &CallNode<'_>) -> Option<ruby_source::Span> {
    if call.is_safe_navigation() || call.name().as_slice() != b"override" {
        return None;
    }
    if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
        return None;
    }
    let arguments = call.arguments()?;
    let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
    let [only] = args.as_slice() else { return None };
    let elements: Vec<Node<'_>> = if let Some(hash) = only.as_hash_node() {
        hash.elements().iter().collect()
    } else {
        only.as_keyword_hash_node()?.elements().iter().collect()
    };
    elements.iter().find_map(|element| {
        let pair = element.as_assoc_node()?;
        let key = pair.key();
        let key = key.as_symbol_node()?;
        (key.unescaped() == b"allow_incompatible" && pair.value().as_true_node().is_some())
            .then(|| element.span())
    })
}
