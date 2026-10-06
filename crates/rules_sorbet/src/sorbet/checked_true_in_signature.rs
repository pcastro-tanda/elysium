//! `Sorbet/CheckedTrueInSignature`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signatures/checked_true_in_signature.rs`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeKind};
use ruby_source::Span;

const MESSAGE: &str = "Using `checked(true)` in a method signature definition is not allowed. `checked(true)` is the default behavior for modules/classes with runtime checks enabled. To enable typechecking at runtime for this module, regardless of global settings, `include(WaffleCone::RuntimeChecks)` to this module and set other methods to `checked(false)`.";

/// Disallows the usage of `checked(true)` in signatures.
#[derive(Debug, Clone)]
pub struct CheckedTrueInSignature;

impl Rule for CheckedTrueInSignature {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/CheckedTrueInSignature",
        department: Department::Sorbet,
        summary: "Disallows the usage of `checked(true)` in signatures.",
        explanation: "Disallows the usage of `checked(true)`.\n\n```ruby\n# bad\nsig { void.checked(true) }\n\n# good\nsig { void }\n```",
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
        if !is_signature(&call) {
            return;
        }
        // `offending_node(node).first`: pre-order search.
        let mut found: Option<(Span, u32)> = None;
        each_descendant(node, &mut |n: &Node<'_>| {
            if found.is_some() {
                return;
            }
            let Some(c) = n.as_call_node() else { return };
            if c.is_safe_navigation() || c.name().as_slice() != b"checked" {
                return;
            }
            if c.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
                return;
            }
            let Some(arguments) = c.arguments() else { return };
            let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
            let [only] = args.as_slice() else { return };
            if only.as_true_node().is_none() {
                return;
            }
            // `error.location.selector.begin_pos..error.location.end.begin_pos`
            let (Some(selector), Some(end)) = (c.message_loc(), c.closing_loc()) else { return };
            found = Some((
                Span::new(selector.span().start, end.span().start),
                ctx.line_col(c.location().span().start).line,
            ));
        });
        let Some((range, line)) = found else { return };
        // `source_range(buffer, line, begin..end)` takes the range as columns
        // of `line`, with the inclusive range's size as the length.
        let begin = ctx.line_span(line).start + range.start;
        let end = begin + (range.end - range.start) + 1;
        if end as usize > ctx.source().bytes().len() {
            return;
        }
        ctx.report(&Self::META, Span::new(begin, end), MESSAGE);
    }
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
