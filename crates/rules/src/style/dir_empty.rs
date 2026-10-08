//! `Style/DirEmpty`, ported from RuboCop's
//! `lib/rubocop/cop/style/dir_empty.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG_TEMPLATE: &str = "Use `%<replacement>s` instead.";

/// Prefer to use `Dir.empty?('path/to/dir')` when checking if a directory is empty.
#[derive(Debug, Clone)]
pub struct DirEmpty;

impl Rule for DirEmpty {
    const META: RuleMeta = RuleMeta {
        name: "Style/DirEmpty",
        department: Department::Style,
        summary: "Prefer to use `Dir.empty?('path/to/dir')` when checking if a directory is empty.",
        explanation: "",
        enabled_by_default: false,
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
        let method = call.name().as_slice();
        if !matches!(method, b"==" | b"!=" | b">" | b"empty?" | b"none?") {
            return;
        }
        if call.block().is_some() {
            return; // a trailing block changes the meaning.
        }
        let Some((const_node, arg_node)) = offensive(&call) else { return };

        let bang = matches!(method, b"!=" | b">");
        let replacement = format!(
            "{}{}.empty?({})",
            if bang { "!" } else { "" },
            String::from_utf8_lossy(ctx.text(const_node.span())),
            String::from_utf8_lossy(ctx.text(arg_node.span())),
        );
        let message = MSG_TEMPLATE.replace("%<replacement>s", &replacement);
        let edits = vec![Edit::replace(node.span(), replacement.into_bytes())];
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// The `Dir` constant node and single argument for one of the four
/// offensive shapes, if `node` matches.
fn offensive<'pr>(node: &CallNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    let method = node.name().as_slice();

    match method {
        b"==" | b"!=" | b">" => {
            // `(send (send (send Dir :entries arg) :size) {== != >} (int 2))`
            // or `(send (send (send Dir :children arg) :size) {== != >} (int 0))`.
            let args = node.arguments()?.arguments();
            if args.len() != 1 {
                return None;
            }
            let arg = args.iter().next()?;
            let expected: &[u8] = if is_int_literal(&arg, 2) {
                b"entries"
            } else if is_int_literal(&arg, 0) {
                b"children"
            } else {
                return None;
            };
            let size_call = node.receiver()?.as_call_node()?;
            if size_call.is_safe_navigation() || size_call.name().as_slice() != b"size" {
                return None;
            }
            let inner = size_call.receiver()?.as_call_node()?;
            dir_call(&inner, expected)
        }
        b"empty?" => {
            // `(send (send Dir :children arg) :empty?)`
            let inner = node.receiver()?.as_call_node()?;
            dir_call(&inner, b"children")
        }
        b"none?" => {
            // `(send (send Dir :each_child arg) :none?)`
            if node.arguments().is_some_and(|a| !a.arguments().is_empty()) {
                return None;
            }
            let inner = node.receiver()?.as_call_node()?;
            dir_call(&inner, b"each_child")
        }
        _ => None,
    }
}

/// `(send (const {nil? cbase} :Dir) method arg)`: a `Dir.<method>(arg)` call
/// with exactly one argument, not safe-navigated.
fn dir_call<'pr>(call: &CallNode<'pr>, method: &[u8]) -> Option<(Node<'pr>, Node<'pr>)> {
    if call.is_safe_navigation() || call.name().as_slice() != method {
        return None;
    }
    let receiver = call.receiver()?;
    let is_dir = receiver.as_constant_read_node().is_some_and(|r| r.name().as_slice() == b"Dir")
        || receiver.as_constant_path_node().is_some_and(|p| {
            p.parent().is_none() && p.name().is_some_and(|n| n.as_slice() == b"Dir")
        });
    if !is_dir {
        return None;
    }
    let args = call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    Some((receiver, args.iter().next()?))
}

/// `node` is a literal integer equal to `value`.
fn is_int_literal(node: &Node<'_>, value: i32) -> bool {
    node.as_integer_node().and_then(|i| i.value().try_into().ok()).is_some_and(|v: i32| v == value)
}
