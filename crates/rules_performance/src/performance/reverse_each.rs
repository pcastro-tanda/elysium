//! `Performance/ReverseEach`, ported from rubocop-performance's
//! `lib/rubocop/cop/performance/reverse_each.rb`.
//!
//! `use_return_value?` looks for an assignment, `send` or `return` ancestor.
//! In whitequark a call that owns a block is wrapped by the block node, so
//! it is not an ancestor of what its block holds; the rule keeps its own
//! ancestor stack to reproduce that.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

const MSG: &str = "Use `reverse_each` instead of `reverse.each`.";

/// Use `reverse_each` instead of `reverse.each`.
#[derive(Debug, Clone)]
pub struct ReverseEach;

/// What `use_return_value?` needs to know about one ancestor.
#[derive(Debug, Clone, Copy)]
struct Anc {
    kind: NodeKind,
    /// A `CallNode` that is a plain (non-`&.`) `send` in whitequark terms.
    send: bool,
    /// For a call: it owns a block literal (so the block node wraps it).
    has_block: bool,
}

struct Collector {
    stack: Vec<Anc>,
    offenses: Vec<Span>,
}

fn is_assignment(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::ReturnNode) || format!("{kind:?}").ends_with("WriteNode")
}

impl Collector {
    fn use_return_value(&self) -> bool {
        let mut i = self.stack.len();
        while i > 0 {
            i -= 1;
            let anc = self.stack[i];
            if is_assignment(anc.kind) {
                return true;
            }
            if anc.send {
                // A call whose block literal holds the node is the block's
                // callee, not a `send` ancestor.
                let inside_block = anc.has_block
                    && self.stack.get(i + 1).is_some_and(|n| n.kind == NodeKind::BlockNode);
                if !inside_block {
                    return true;
                }
            }
        }
        false
    }
}

impl<'pr> Visitor<'pr> for Collector {
    fn enter(&mut self, node: &Node<'pr>) {
        let mut anc = Anc { kind: node.kind(), send: false, has_block: false };
        if let Some(each) = node.as_call_node() {
            anc.send = !each.is_safe_navigation();
            anc.has_block = each.block().is_some_and(|b| b.as_block_node().is_some());
            if each.name().as_slice() == b"each"
                && each.arguments().is_none()
                && each.block().is_none_or(|b| b.as_block_node().is_some())
            {
                if let Some(reverse) = each.receiver().and_then(|r| r.as_call_node()) {
                    if reverse.name().as_slice() == b"reverse"
                        && reverse.arguments().is_none()
                        && reverse.block().is_none()
                        && !self.use_return_value()
                    {
                        if let (Some(rev_sel), Some(each_sel)) =
                            (reverse.message_loc(), each.message_loc())
                        {
                            self.offenses
                                .push(Span::new(rev_sel.span().start, each_sel.span().end));
                        }
                    }
                }
            }
        }
        self.stack.push(anc);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

impl Rule for ReverseEach {
    const META: RuleMeta = RuleMeta {
        name: "Performance/ReverseEach",
        department: Department::Performance,
        summary: "Use `reverse_each` instead of `reverse.each`.",
        explanation: "Identifies usages of `reverse.each` and changes them to use `reverse_each` \
                      instead. If the return value is used, it will not be detected because the \
                      result will be different.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let mut collector = Collector { stack: Vec::new(), offenses: Vec::new() };
        walk(&ctx.parsed().root(), &mut collector);
        for span in collector.offenses {
            ctx.report_with_fix(
                &Self::META,
                span,
                MSG,
                Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, b"reverse_each".to_vec())],
                },
            );
        }
    }
}
