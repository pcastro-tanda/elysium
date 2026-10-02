//! `Style/PredicateWithKind`, ported from RuboCop's
//! `lib/rubocop/cop/style/predicate_with_kind.rb`.
//!
//! whitequark represents a numbered-/`it`-parameter block's single implicit
//! parameter (`_1`/`it`) the same way it represents an explicit named one --
//! as a plain `lvar` read inside the body, which is why upstream's
//! `kind_call?` pattern (`(send (lvar %1) ...)`) works unchanged across all
//! three block shapes once `block_arg_name` has resolved the right name.
//! Prism instead gives a bare `_1` a regular [`NodeKind::LocalVariableReadNode`]
//! but a bare `it` its own dedicated [`NodeKind::ItLocalVariableReadNode`] (or,
//! ambiguously, a zero-argument [`NodeKind::CallNode`] named `it` when the
//! parser cannot tell them apart) -- [`is_arg_ref`] checks both shapes for
//! `it`, matching `redundant_safe_navigation.rs`'s identical `binding_name`
//! helper.
//!
//! whitequark's `block_node.body&.begin_type?` (true for a multi-statement
//! body, since whitequark elides the wrapping `begin` only for a single
//! statement) becomes a direct `StatementsNode::body().len() != 1` check:
//! Prism always wraps a block body in a `StatementsNode` regardless of
//! statement count.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Looks for uses of `any?`, `all?`, `none?`, or `one?` with a block
/// containing only an `is_a?`, `kind_of?`, or `instance_of?` check, and
/// suggests using the predicate method with the class argument directly.
#[derive(Debug, Clone)]
pub struct PredicateWithKind;

impl Rule for PredicateWithKind {
    const META: RuleMeta = RuleMeta {
        name: "Style/PredicateWithKind",
        department: Department::Style,
        summary: "Prefer `any?(Klass)` to `any? { |x| x.is_a?(Klass) }`.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if !matches!(call.name().as_slice(), b"any?" | b"all?" | b"none?" | b"one?") {
            return;
        }
        let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
        let Some(arg_name) = block_arg_name(&block) else { return };
        let Some(kind_call) = kind_check_call(&block, &arg_name) else { return };
        let Some(klass) = kind_call.arguments().and_then(|a| a.arguments().iter().next()) else {
            return;
        };
        let Some(message_loc) = call.message_loc() else { return };

        let method_name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let klass_src = String::from_utf8_lossy(ctx.text(klass.span())).into_owned();
        let replacement = format!("{method_name}({klass_src})");
        let message =
            format!("Prefer `{replacement}` to `{method_name} {{ ... }}` with a kind check.");

        let span = node.span();
        let edit_span = Span::new(message_loc.span().start, block.closing_loc().span().end);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(edit_span, replacement.into_bytes())],
            },
        );
    }
}

/// RuboCop's `kind_check?`'s block-arg-name resolution: the name a plain
/// explicit block parameter, a numbered block's sole `_1`, or an `it` block
/// uses for the kind check's receiver. `None` for any other parameter shape
/// (no parameters at all, more than one, or anything but a plain required
/// parameter).
fn block_arg_name(block: &BlockNode<'_>) -> Option<Vec<u8>> {
    let params = block.parameters()?;
    match params.kind() {
        NodeKind::BlockParametersNode => {
            let block_params = params.as_block_parameters_node()?;
            let parameters = block_params.parameters()?;
            if !parameters.optionals().is_empty()
                || parameters.rest().is_some()
                || !parameters.posts().is_empty()
                || !parameters.keywords().is_empty()
                || parameters.keyword_rest().is_some()
                || parameters.block().is_some()
            {
                return None;
            }
            let requireds: Vec<Node<'_>> = parameters.requireds().iter().collect();
            let [only] = requireds.as_slice() else { return None };
            Some(only.as_required_parameter_node()?.name().as_slice().to_vec())
        }
        NodeKind::NumberedParametersNode => {
            let numbered = params.as_numbered_parameters_node()?;
            (numbered.maximum() == 1).then(|| b"_1".to_vec())
        }
        NodeKind::ItParametersNode => Some(b"it".to_vec()),
        _ => None,
    }
}

/// RuboCop's `kind_check?` + `kind_call?`, combined: the block's single
/// statement, if it is exactly `(send (lvar arg_name) {is_a? kind_of?
/// instance_of?} _)`.
fn kind_check_call<'pr>(block: &BlockNode<'pr>, arg_name: &[u8]) -> Option<CallNode<'pr>> {
    let body = block.body()?;
    let stmts = body.as_statements_node()?;
    let items: Vec<Node<'_>> = stmts.body().iter().collect();
    let [stmt] = items.as_slice() else { return None };
    let call = stmt.as_call_node()?;
    if !matches!(call.name().as_slice(), b"is_a?" | b"kind_of?" | b"instance_of?") {
        return None;
    }
    let receiver = call.receiver()?;
    if !is_arg_ref(&receiver, arg_name) {
        return None;
    }
    if call.arguments()?.arguments().len() != 1 {
        return None;
    }
    Some(call)
}

/// Is `node` a read of the block's implicit/explicit argument named
/// `arg_name`? See the module docs for the `it`-specific ambiguity.
fn is_arg_ref(node: &Node<'_>, arg_name: &[u8]) -> bool {
    if arg_name == b"it" {
        if node.kind() == NodeKind::ItLocalVariableReadNode {
            return true;
        }
        return node.as_call_node().is_some_and(|c| {
            c.receiver().is_none() && c.arguments().is_none() && c.name().as_slice() == b"it"
        });
    }
    node.as_local_variable_read_node().is_some_and(|lv| lv.name().as_slice() == arg_name)
}
