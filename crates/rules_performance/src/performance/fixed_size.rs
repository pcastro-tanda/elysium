//! `Performance/FixedSize`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/fixed_size.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Do not compute the size of statically sized objects.";

/// Do not compute the size of statically sized objects except in constants.
#[derive(Debug, Clone, Default)]
pub struct FixedSize {
    /// Number of enclosing constant assignments / blocks (whitequark `casgn`
    /// and `any_block` ancestors), including a call's own block.
    allowed_ancestors: u32,
}

/// Whether `node` is an `allowed_parent?` ancestor for everything it encloses.
/// A whitequark `block` also wraps the call it is attached to, so the call
/// (receiver, arguments and block) counts as inside it.
fn is_allowed_parent(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::LambdaNode | NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode => {
            true
        }
        NodeKind::CallNode => node
            .as_call_node()
            .is_some_and(|call| call.block().is_some_and(|b| b.as_block_node().is_some())),
        _ => false,
    }
}

impl Rule for FixedSize {
    const META: RuleMeta = RuleMeta {
        name: "Performance/FixedSize",
        department: Department::Performance,
        summary: "Do not compute the size of statically sized objects except in constants.",
        explanation: "Do not compute the size of statically sized objects.\n\n```ruby\n# bad\n'foo'.size\n%q[bar].count\n:fred.size\n[1, 2, thud].count\n{ a: corge, b: grault }.length\n\n# good\nfoo.size\n:\"#{fred}\".size\nCONST = :baz.length\n[1, 2, *thud].count\n{ a: corge, **grault }.length\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::LambdaNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if is_allowed_parent(node) {
            self.allowed_ancestors += 1;
        }
        if self.allowed_ancestors > 0 {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if !matches!(call.name().as_slice(), b"count" | b"length" | b"size") {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        match receiver.kind() {
            NodeKind::ArrayNode => {
                let Some(array) = receiver.as_array_node() else { return };
                if array.elements().iter().any(|e| e.as_splat_node().is_some()) {
                    return;
                }
            }
            NodeKind::HashNode => {
                let Some(hash) = receiver.as_hash_node() else { return };
                if hash.elements().iter().any(|e| e.as_assoc_splat_node().is_some()) {
                    return;
                }
            }
            NodeKind::StringNode | NodeKind::SymbolNode => {}
            _ => return,
        }
        // `allowed_argument?`: a first argument (incl. `&blk`) that is not a `str`.
        let first = match call.arguments() {
            Some(args) => args.arguments().iter().next(),
            None => call.block().filter(|b| b.as_block_argument_node().is_some()),
        };
        if first.is_some_and(|arg| arg.as_string_node().is_none()) {
            return;
        }
        ctx.report(&Self::META, ruby_ast::ext::call_span_excluding_block(&call), MSG);
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if is_allowed_parent(node) {
            self.allowed_ancestors -= 1;
        }
    }
}
