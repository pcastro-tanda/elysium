//! `Sorbet/VoidCheckedTests`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/signatures/void_checked_tests.rs`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::const_name;
use ruby_ast::node::{BlockNode, CallNode};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeKind};

const MESSAGE: &str = "Returning `.void` from a sig marked `.checked(:tests)` means that the method will return a different value in non-test environments (possibly with different truthiness). Either use `.returns(T.anything).checked(:tests)` to keep checking in tests, or `.void.checked(:never)` to leave it untouched.";

/// Disallows the usage of `.void.checked(:tests)`.
#[derive(Debug, Clone)]
pub struct VoidCheckedTests;

impl Rule for VoidCheckedTests {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/VoidCheckedTests",
        department: Department::Sorbet,
        summary: "Forbid `.void.checked(:tests)`",
        explanation: "Disallows the usage of `.void.checked(:tests)`.\n\n```ruby\n# bad\nsig { void.checked(:tests) }\n\n# good\nsig { void }\nsig { returns(T.anything).checked(:tests) }\nsig { void.checked(:never) }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode, NodeKind::StatementsNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(statements) = node.as_statements_node() {
            // Signatures that are statements know their later siblings.
            let body: Vec<Node<'_>> = statements.body().iter().collect();
            for (index, statement) in body.iter().enumerate() {
                let Some(call) = statement.as_call_node() else { continue };
                if is_signature(&call) {
                    check(&call, statement, &body[index + 1..], ctx);
                }
            }
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if !is_signature(&call) {
            return;
        }
        if ctx.parent().is_some_and(|p| p.kind == NodeKind::StatementsNode) {
            return;
        }
        check(&call, node, &[], ctx);
    }
}

fn check(call: &CallNode<'_>, node: &Node<'_>, later: &[Node<'_>], ctx: &mut Context<'_>) {
    // `checked_tests(node).first`
    let mut checked = false;
    each_descendant(node, &mut |n: &Node<'_>| {
        let Some(c) = n.as_call_node() else { return };
        if c.name().as_slice() != b"checked"
            || c.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(arguments) = c.arguments() else { return };
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [only] = args.as_slice() else { return };
        if only.as_symbol_node().is_some_and(|s| s.unescaped() == b"tests") {
            checked = true;
        }
    });
    if !checked {
        return;
    }

    if let Some(def) = later.iter().find_map(Node::as_def_node) {
        // Sorbet requires that `initialize` methods return `.void`.
        if def.name().as_slice() == b"initialize" {
            return;
        }
    }

    let Some(block) = call.block().and_then(|b| b.as_block_node()) else { return };
    let Some(body) = single_body(&block) else { return };
    let Some(void_send) = top_level_void(&body) else { return };
    let Some(selector) = void_send.message_loc() else { return };
    let span = selector.span();
    ctx.report_with_fix(
        &VoidCheckedTests::META,
        span,
        MESSAGE,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, b"returns(T.anything)".to_vec())],
        },
    );
}

fn top_level_void<'a>(node: &Node<'a>) -> Option<CallNode<'a>> {
    let call = node.as_call_node()?;
    if !is_send_like(&call) {
        return None;
    }
    if call.name().as_slice() == b"void" {
        Some(call)
    } else {
        top_level_void(&call.receiver()?)
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

/// The block's body when it is a single statement (whitequark elides the
/// `begin`); `None` for no body or several statements.
fn single_body<'a>(block: &BlockNode<'a>) -> Option<Node<'a>> {
    let body = block.body()?;
    let statements = body.as_statements_node()?;
    let mut iter = statements.body().iter();
    let first = iter.next()?;
    iter.next().is_none().then_some(first)
}

/// A `send`/`csend` node: a call without a literal block.
fn is_send_like(call: &CallNode<'_>) -> bool {
    call.block().is_none_or(|b| b.as_block_node().is_none())
}
