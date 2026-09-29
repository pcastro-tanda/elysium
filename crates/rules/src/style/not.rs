//! `Style/Not`, ported from RuboCop's
//! `lib/rubocop/cop/style/not.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Side;

const MSG: &str = "Use `!` instead of `not`.";

/// Use ! instead of not.
#[derive(Debug, Clone)]
pub struct Not;

impl Rule for Not {
    const META: RuleMeta = RuleMeta {
        name: "Style/Not",
        department: Department::Style,
        summary: "Use ! instead of not.",
        explanation: "\
Checks for uses of the keyword `not` instead of `!`.
The `not` keyword has lower precedence than `!`, which can
lead to surprising behavior and often requires parentheses.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
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
        if call.name().as_slice() != b"!" {
            return;
        }
        let Some(message_loc) = call.message_loc() else { return };
        if ctx.text(message_loc.span()) != b"not" {
            return;
        }

        let selector_span = message_loc.span();
        let range = ctx.with_surrounding_space(selector_span, Side::Right, true, false);
        let receiver = call.receiver();

        let opposite = receiver.as_ref().and_then(opposite_method);
        let edits = if let Some((recv_call, opposite)) = opposite {
            let selector = recv_call.message_loc().expect("comparison call always has a selector");
            vec![Edit::delete(range), Edit::replace(selector.span(), opposite.as_bytes().to_vec())]
        } else if receiver.as_ref().is_some_and(requires_parens) {
            vec![Edit::replace(range, b"!(".to_vec()), Edit::insert(node.span().end, b")".to_vec())]
        } else {
            vec![Edit::replace(range, b"!".to_vec())]
        };

        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, selector_span, MSG, fix);
    }
}

/// RuboCop's `opposite_method?` gate plus the `OPPOSITE_METHODS` lookup: a
/// comparison `CallNode` whose operator has a direct opposite, returned
/// alongside the receiver re-typed as a `CallNode` for its selector.
fn opposite_method<'pr>(child: &Node<'pr>) -> Option<(CallNode<'pr>, &'static str)> {
    let call = child.as_call_node()?;
    let opposite = match call.name().as_slice() {
        b"==" => "!=",
        b"!=" => "==",
        b"<=" => ">",
        b">" => "<=",
        b"<" => ">=",
        b">=" => "<",
        _ => return None,
    };
    Some((call, opposite))
}

/// RuboCop's `requires_parens?`.
fn requires_parens(child: &Node<'_>) -> bool {
    match child.kind() {
        NodeKind::AndNode | NodeKind::OrNode | NodeKind::RangeNode | NodeKind::FlipFlopNode => true,
        NodeKind::CallNode => {
            let call = child.as_call_node().expect("kind matched");
            is_binary_operation(&call)
        }
        NodeKind::IfNode => child.as_if_node().expect("kind matched").if_keyword_loc().is_none(),
        kind => is_assignment(kind),
    }
}

/// RuboCop-AST's `MethodDispatchNode#binary_operation?`: an operator-method
/// call (`OPERATOR_METHODS`) whose own span starts before its selector --
/// i.e. it has a textual receiver, ruling out unary prefix forms (`-@`,
/// `+@`, `~`, `!`) whose span and selector coincide.
fn is_binary_operation(call: &CallNode<'_>) -> bool {
    if call.receiver().is_none() {
        return false;
    }
    if !is_operator_method(call.name().as_slice()) {
        return false;
    }
    let Some(message_loc) = call.message_loc() else { return false };
    call.as_node().span().start != message_loc.span().start
}

/// `MethodIdentifierPredicates::OPERATOR_METHODS`.
fn is_operator_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"|" | b"^"
            | b"&"
            | b"<=>"
            | b"=="
            | b"==="
            | b"=~"
            | b">"
            | b">="
            | b"<"
            | b"<="
            | b"<<"
            | b">>"
            | b"+"
            | b"-"
            | b"*"
            | b"/"
            | b"%"
            | b"**"
            | b"~"
            | b"+@"
            | b"-@"
            | b"!@"
            | b"~@"
            | b"[]"
            | b"[]="
            | b"!"
            | b"!="
            | b"!~"
            | b"`"
    )
}

/// RuboCop-AST's `ASSIGNMENTS`: equals-assignments (`lvasgn`/`ivasgn`/
/// `cvasgn`/`gvasgn`/`casgn`/`masgn`) plus shorthand assignments
/// (`op_asgn`/`or_asgn`/`and_asgn`), regardless of the target being a
/// variable, constant, method call, or index -- Prism splits each of these
/// into a per-target `*WriteNode` kind where whitequark uses one generic
/// node type per assignment shape.
fn is_assignment(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::LocalVariableWriteNode
            | NodeKind::InstanceVariableWriteNode
            | NodeKind::ClassVariableWriteNode
            | NodeKind::GlobalVariableWriteNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
            | NodeKind::MultiWriteNode
            | NodeKind::LocalVariableAndWriteNode
            | NodeKind::InstanceVariableAndWriteNode
            | NodeKind::ClassVariableAndWriteNode
            | NodeKind::GlobalVariableAndWriteNode
            | NodeKind::ConstantAndWriteNode
            | NodeKind::ConstantPathAndWriteNode
            | NodeKind::CallAndWriteNode
            | NodeKind::IndexAndWriteNode
            | NodeKind::LocalVariableOrWriteNode
            | NodeKind::InstanceVariableOrWriteNode
            | NodeKind::ClassVariableOrWriteNode
            | NodeKind::GlobalVariableOrWriteNode
            | NodeKind::ConstantOrWriteNode
            | NodeKind::ConstantPathOrWriteNode
            | NodeKind::CallOrWriteNode
            | NodeKind::IndexOrWriteNode
            | NodeKind::LocalVariableOperatorWriteNode
            | NodeKind::InstanceVariableOperatorWriteNode
            | NodeKind::ClassVariableOperatorWriteNode
            | NodeKind::GlobalVariableOperatorWriteNode
            | NodeKind::ConstantOperatorWriteNode
            | NodeKind::ConstantPathOperatorWriteNode
            | NodeKind::CallOperatorWriteNode
            | NodeKind::IndexOperatorWriteNode
    )
}
