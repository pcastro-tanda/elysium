//! `Performance/Size`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/size.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

const MSG: &str = "Use `size` instead of `count`.";

/// Use `size` instead of `count` for counting the number of elements in `Array` and `Hash`.
#[derive(Debug, Clone)]
pub struct Size;

impl Rule for Size {
    const META: RuleMeta = RuleMeta {
        name: "Performance/Size",
        department: Department::Performance,
        summary: "Use `size` instead of `count` for counting the number of elements in `Array` and `Hash`.",
        explanation: "\
Identifies usages of `count` on an `Array` and `Hash` and change them to `size`.

```ruby
# bad
[1, 2, 3].count
(1..3).to_a.count
Array[*1..3].count
Array(1..3).count

# bad
{a: 1, b: 2, c: 3}.count
[[:foo, :bar], [1, 2]].to_h.count
Hash[*('a'..'z')].count
Hash(key: :value).count

# good
[1, 2, 3].size
(1..3).to_a.size
Array[*1..3].size
Array(1..3).size

# good
{a: 1, b: 2, c: 3}.size
[[:foo, :bar], [1, 2]].to_h.size
Hash[*('a'..'z')].size
Hash(key: :value).size

# good
[1, 2, 3].count { |e| e > 2 }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
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
        if call.name().as_slice() != b"count"
            || call.arguments().is_some()
            || call.block().is_some()
        {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if !(is_array(&receiver) || is_hash(&receiver)) || parent_is_block(ctx, node) {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(span, b"size".to_vec())] },
        );
    }
}

/// `node.parent&.block_type?`: the node is the sole statement of a block or
/// lambda body (a call that is itself the block's call carries a `block()`).
fn parent_is_block(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    let ancestors = ctx.ancestors();
    let [.., owner, stmts] = ancestors else { return false };
    stmts.kind == NodeKind::StatementsNode
        && stmts.span == node.span()
        && matches!(owner.kind, NodeKind::BlockNode | NodeKind::LambdaNode)
}

fn arg_count(call: &CallNode<'_>) -> usize {
    call.arguments().map_or(0, |a| a.arguments().iter().count())
        + usize::from(call.block().is_some_and(|b| b.as_block_argument_node().is_some()))
}

/// `(call _ <name>)`: no arguments, no block.
fn bare_call(call: &CallNode<'_>, name: &[u8]) -> bool {
    call.name().as_slice() == name && arg_count(call) == 0 && call.block().is_none()
}

/// `(send <receiver> <name> _)`.
fn one_arg_call(call: &CallNode<'_>, name: &[u8]) -> bool {
    !call.is_safe_navigation()
        && call.name().as_slice() == name
        && arg_count(call) == 1
        && call.block().is_none_or(|b| b.as_block_argument_node().is_some())
}

fn is_const_named(node: &Node<'_>, name: &[u8]) -> bool {
    node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == name)
}

fn is_array(node: &Node<'_>) -> bool {
    if node.as_array_node().is_some() {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    bare_call(&call, b"to_a")
        || (one_arg_call(&call, b"[]")
            && call.receiver().is_some_and(|r| is_const_named(&r, b"Array")))
        || (one_arg_call(&call, b"Array") && call.receiver().is_none())
}

fn is_hash(node: &Node<'_>) -> bool {
    if node.as_hash_node().is_some() {
        return true;
    }
    let Some(call) = node.as_call_node() else { return false };
    bare_call(&call, b"to_h")
        || (one_arg_call(&call, b"[]")
            && call.receiver().is_some_and(|r| is_const_named(&r, b"Hash")))
        || (one_arg_call(&call, b"Hash") && call.receiver().is_none())
}
