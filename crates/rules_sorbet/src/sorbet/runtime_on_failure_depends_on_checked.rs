//! `Sorbet/RuntimeOnFailureDependsOnChecked`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signatures/runtime_on_failure_depends_on_checked.rs`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

const MSG: &str = "To use .on_failure you must additionally call .checked(:tests) or .checked(:always), otherwise, the .on_failure has no effect.";

/// Checks that `on_failure` is not used without `checked(:tests)` or
/// `checked(:always)`.
#[derive(Debug, Clone)]
pub struct RuntimeOnFailureDependsOnChecked;

impl Rule for RuntimeOnFailureDependsOnChecked {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/RuntimeOnFailureDependsOnChecked",
        department: Department::Sorbet,
        summary: "Ensures that `on_failure` is called after `checked` in signatures.",
        explanation: "Checks that `on_failure` is not used without `checked(:tests)` or `checked(:always)`.\n\n```ruby\n# bad\nsig { params(x: Integer).returns(Integer).on_failure(:raise) }\n\n# good\nsig { params(x: Integer).returns(Integer).checked(:always).on_failure(:raise) }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if !is_signature(&call) {
            return;
        }
        let mut on_failure = false;
        let mut checked = false;
        each_descendant(node, &mut |n: &Node<'_>| {
            let Some(c) = n.as_call_node() else { return };
            if c.is_safe_navigation() {
                return;
            }
            if c.name().as_slice() == b"on_failure" {
                on_failure = true;
            }
            if c.name().as_slice() == b"checked" && is_checked_tests_or_always(&c) {
                checked = true;
            }
        });
        if !on_failure || checked {
            return;
        }
        ctx.report(&Self::META, node.span(), MSG);
    }
}

/// `(send _ :checked (sym {:tests | :always}))`.
fn is_checked_tests_or_always(call: &CallNode<'_>) -> bool {
    if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
    let [only] = args.as_slice() else { return false };
    only.as_symbol_node().is_some_and(|s| matches!(s.unescaped(), b"tests" | b"always"))
}

/// `Sorbet::SignatureHelp#signature?`: `sig`, `sig(:final)`, `T::Sig.sig`...
/// called with a literal block taking no parameters (whitequark `block`, not
/// `numblock`/`itblock`).
fn is_signature(call: &CallNode<'_>) -> bool {
    if call.is_safe_navigation() || call.name().as_slice() != b"sig" {
        return false;
    }
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    if let Some(params) = block.parameters() {
        let Some(params) = params.as_block_parameters_node() else { return false };
        if params.parameters().is_some() || params.locals().iter().next().is_some() {
            return false;
        }
    }
    if let Some(arguments) = call.arguments() {
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [only] = args.as_slice() else { return false };
        let Some(sym) = only.as_symbol_node() else { return false };
        if sym.unescaped() != b"final" {
            return false;
        }
    }
    match call.receiver() {
        None => true,
        Some(receiver) => {
            matches!(const_name(&receiver).as_deref(), Some("T::Sig" | "T::Sig::WithoutRuntime"))
        }
    }
}
