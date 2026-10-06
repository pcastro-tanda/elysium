//! `Sorbet/BindingConstantWithoutTypeAlias`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/binding_constant_without_type_alias.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "It looks like you're trying to bind a type to a constant. \
                   To do this, you must alias the type using `T.type_alias`.";
const WITHOUT_BLOCK_MSG: &str = "It looks like you're using the old `T.type_alias` syntax. \
                                 `T.type_alias` now expects a block.\
                                 Run Sorbet with the options \"--autocorrect --error-white-list=5043\" \
                                 to automatically upgrade to the new syntax.";

const REQUIRES_TYPE_ALIAS: [&[u8]; 8] =
    [b"all", b"any", b"class_of", b"nilable", b"noreturn", b"proc", b"self_type", b"untyped"];

/// Disallows binding the return value of `T.any`, `T.all`, `T.enum` to a constant directly. To bind the value, one must use `T.type_alias`.
#[derive(Debug, Clone)]
pub struct BindingConstantWithoutTypeAlias;

impl Rule for BindingConstantWithoutTypeAlias {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/BindingConstantWithoutTypeAlias",
        department: Department::Sorbet,
        summary: "Disallows binding the return value of `T.any`, `T.all`, `T.enum` to a constant directly. To bind the value, one must use `T.type_alias`.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ConstantWriteNode, NodeKind::ConstantPathWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let expression = if let Some(write) = node.as_constant_write_node() {
            write.value()
        } else if let Some(write) = node.as_constant_path_write_node() {
            write.value()
        } else {
            return;
        };
        let span = expression.span();

        if let Some(type_arg) = type_alias_without_block(&expression) {
            let replacement = format!(
                "T.type_alias {{ {} }}",
                String::from_utf8_lossy(ctx.text(type_arg.span()))
            );
            ctx.report_with_fix(
                &Self::META,
                span,
                WITHOUT_BLOCK_MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, replacement.into_bytes())],
                },
            );
            return;
        }

        if type_alias_with_block(&expression) {
            return;
        }

        if requires_type_alias(&send_leaf(expression)) {
            let replacement =
                format!("T.type_alias {{ {} }}", String::from_utf8_lossy(ctx.text(span)));
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, replacement.into_bytes())],
                },
            );
        }
    }
}

/// `(const {nil? cbase} :T)`.
fn is_t_const(node: &Node<'_>) -> bool {
    if let Some(c) = node.as_constant_read_node() {
        return c.name().as_slice() == b"T";
    }
    node.as_constant_path_node()
        .is_some_and(|p| p.parent().is_none() && p.name().is_some_and(|n| n.as_slice() == b"T"))
}

fn has_block_node(call: &ruby_ast::node::CallNode<'_>) -> bool {
    call.block().is_some_and(|b| b.as_block_node().is_some())
}

/// A whitequark `send` node: neither `csend` nor wrapped in a `block`.
fn as_send<'a>(node: &Node<'a>) -> Option<ruby_ast::node::CallNode<'a>> {
    let call = node.as_call_node()?;
    (!call.is_safe_navigation() && !has_block_node(&call)).then_some(call)
}

/// rubocop-ast's `Node#receiver`: `{(send $_ ...) (any_block (call $_ ...) ...)}`.
fn receiver_of<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let call = node.as_call_node()?;
    if has_block_node(&call) || !call.is_safe_navigation() {
        call.receiver()
    } else {
        None
    }
}

fn send_leaf<'a>(mut node: Node<'a>) -> Node<'a> {
    while let Some(receiver) = receiver_of(&node) {
        if as_send(&receiver).is_none() {
            break;
        }
        node = receiver;
    }
    node
}

/// `(send (const {nil? cbase} :T) :type_alias $_)`.
fn type_alias_without_block<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let call = as_send(node)?;
    if call.name().as_slice() != b"type_alias" || !call.receiver().is_some_and(|r| is_t_const(&r)) {
        return None;
    }
    let mut args: Vec<Node<'a>> =
        call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block() {
        args.push(block);
    }
    if args.len() == 1 {
        args.pop()
    } else {
        None
    }
}

/// `(block (send (const {nil? cbase} :T) :type_alias) ...)`.
fn type_alias_with_block(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return false };
    !call.is_safe_navigation()
        && call.arguments().is_none()
        && block.parameters().is_none_or(|p| p.as_block_parameters_node().is_some())
        && call.name().as_slice() == b"type_alias"
        && call.receiver().is_some_and(|r| is_t_const(&r))
}

/// `(send (const {nil? cbase} :T) {:all :any ...} ...)`.
fn requires_type_alias(node: &Node<'_>) -> bool {
    let Some(call) = as_send(node) else { return false };
    REQUIRES_TYPE_ALIAS.contains(&call.name().as_slice())
        && call.receiver().is_some_and(|r| is_t_const(&r))
}
