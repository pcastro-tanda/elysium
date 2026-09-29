//! `Style/RedundantException`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_exception.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG_EXPLODED: &str = "Redundant `RuntimeError` argument can be removed.";
const MSG_COMPACT: &str =
    "Redundant `RuntimeError.new` call can be replaced with just the message.";

/// Checks for `RuntimeError` as the argument of `raise`/`fail`.
#[derive(Debug, Clone)]
pub struct RedundantException;

impl Rule for RedundantException {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantException",
        department: Department::Style,
        summary: "Checks for an obsolete RuntimeException argument in raise/fail.",
        explanation: "Checks for `RuntimeError` as the argument of `raise`/`fail`.\n\n\
# Examples\n\n\
```ruby\n\
# bad\n\
raise RuntimeError, 'message'\n\
raise RuntimeError.new('message')\n\n\
# good\n\
raise 'message'\n\n\
# bad - message is not a string\n\
raise RuntimeError, Object.new\n\
raise RuntimeError.new(Object.new)\n\n\
# good\n\
raise Object.new.to_s\n\
```",
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
        if call.receiver().is_some() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        if name != b"raise" && name != b"fail" {
            return;
        }
        if fix_exploded(&call, node, ctx) {
            return;
        }
        fix_compact(&call, node, ctx);
    }
}

/// Upstream's `exploded?` node matcher: `(send nil? {:raise :fail} (const
/// {nil? cbase} :RuntimeError) $_)`. Switches `raise RuntimeError, 'message'`
/// to `raise 'message'`.
fn fix_exploded(call: &CallNode<'_>, node: &Node<'_>, ctx: &mut Context<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    let items: Vec<Node<'_>> = args.arguments().iter().collect();
    let [exception, message] = items.as_slice() else { return false };
    if !is_runtime_error_const(exception) {
        return false;
    }
    // `raise RuntimeError, nil` uses the class name as the message, so
    // rewriting it to `raise nil.to_s` (an empty message) would change it.
    if message.kind() == NodeKind::NilNode {
        return false;
    }

    let command = String::from_utf8_lossy(name_bytes(call)).into_owned();
    let arg = message_as_expression(ctx, message);
    let arg = if call.opening_loc().is_some() { format!("({arg})") } else { format!(" {arg}") };
    let replacement = format!("{command}{arg}");

    ctx.report_with_fix(
        &RedundantException::META,
        node.span(),
        MSG_EXPLODED,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
        },
    );
    true
}

/// Upstream's `compact?` node matcher: `(send nil? {:raise :fail}
/// $(send (const {nil? cbase} :RuntimeError) :new $_))`. Switches
/// `raise RuntimeError.new('message')` to `raise 'message'`.
fn fix_compact(call: &CallNode<'_>, node: &Node<'_>, ctx: &mut Context<'_>) -> bool {
    let Some(args) = call.arguments() else { return false };
    let items: Vec<Node<'_>> = args.arguments().iter().collect();
    let [only] = items.as_slice() else { return false };
    let Some(new_call) = only.as_call_node() else { return false };
    if new_call.name().as_slice() != b"new" {
        return false;
    }
    let Some(receiver) = new_call.receiver() else { return false };
    if !is_runtime_error_const(&receiver) {
        return false;
    }
    let Some(new_args) = new_call.arguments() else { return false };
    let new_items: Vec<Node<'_>> = new_args.arguments().iter().collect();
    let [message] = new_items.as_slice() else { return false };
    if message.kind() == NodeKind::NilNode {
        return false;
    }

    let replacement = message_as_expression(ctx, message);

    ctx.report_with_fix(
        &RedundantException::META,
        node.span(),
        MSG_COMPACT,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(only.span(), replacement.into_bytes())],
        },
    );
    true
}

/// `command.source` -- always the literal `raise`/`fail` identifier
/// (`RESTRICT_ON_SEND`), never a differently-cased spelling.
fn name_bytes<'a>(call: &'a CallNode<'_>) -> &'a [u8] {
    call.name().as_slice()
}

/// Upstream's `string_message?` (`message.any_str_type?`) plus the `.to_s`
/// fallback from `replaced_exploded`/`replaced_compact`.
fn message_as_expression(ctx: &Context<'_>, message: &Node<'_>) -> String {
    let source = String::from_utf8_lossy(ctx.text(message.span())).into_owned();
    if is_string_message(message) {
        source
    } else {
        format!("{source}.to_s")
    }
}

/// Upstream's `Node#any_str_type?` (`STR_TYPES = %i[str dstr xstr]`, plus
/// their interpolated Prism counterparts).
fn is_string_message(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::StringNode
            | NodeKind::InterpolatedStringNode
            | NodeKind::XStringNode
            | NodeKind::InterpolatedXStringNode
    )
}

/// `rubocop-ast`'s `Node#const_name` special-cased for a bare `RuntimeError`
/// or a top-level `::RuntimeError` -- i.e. the `(const {nil? cbase}
/// :RuntimeError)` node-pattern fragment shared by both matchers. A
/// namespaced `Foo::RuntimeError` does not match.
fn is_runtime_error_const(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"RuntimeError")
        }
        NodeKind::ConstantPathNode => node.as_constant_path_node().is_some_and(|c| {
            c.parent().is_none() && c.name().is_some_and(|n| n.as_slice() == b"RuntimeError")
        }),
        _ => false,
    }
}
