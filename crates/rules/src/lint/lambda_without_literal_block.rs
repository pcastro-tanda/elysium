//! `Lint/LambdaWithoutLiteralBlock`, ported from RuboCop's
//! `lib/rubocop/cop/lint/lambda_without_literal_block.rb`.
//!
//! # Prism shape
//!
//! Whitequark wraps `lambda { ... }` in a `:block` node whose first child is
//! the `send` for `lambda`, so upstream's `node.parent&.block_type?` detects
//! a literal block attached to the call. Prism instead attaches the block
//! directly to the `CallNode` as its `block` field, so this checks whether
//! that field holds a `BlockNode` for the same condition. A `&arg`
//! block-pass argument -- whitequark's ordinary `block_pass` node among the
//! call's regular arguments, matched by upstream's `first_argument` -- is
//! also carried in Prism's `block` field, as a `BlockArgumentNode`, rather
//! than in `arguments()`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `MSG`.
const MSG: &str =
    "lambda without a literal block is deprecated; use the proc without lambda instead.";

/// Checks uses of lambda without a literal block.
#[derive(Debug, Clone)]
pub struct LambdaWithoutLiteralBlock;

impl Rule for LambdaWithoutLiteralBlock {
    const META: RuleMeta = RuleMeta {
        name: "Lint/LambdaWithoutLiteralBlock",
        department: Department::Lint,
        summary: "Checks uses of lambda without a literal block.",
        explanation: "\
```ruby
# bad
lambda(&proc { do_something })
lambda(&Proc.new { do_something })

# good
proc { do_something }
Proc.new { do_something }
lambda { do_something } # If you use lambda.
```",
        enabled_by_default: false,
        severity: Severity::Warning,
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
        if call.receiver().is_some() || call.name().as_slice() != b"lambda" {
            return;
        }
        let block = call.block();
        if block.as_ref().is_some_and(|b| b.kind() == NodeKind::BlockNode) {
            return;
        }
        let first = match &block {
            Some(b) if b.kind() == NodeKind::BlockArgumentNode => Some(*b),
            _ => call.arguments().and_then(|a| a.arguments().first()),
        };
        let Some(first) = first else { return };
        if let Some(block_pass) = first.as_block_argument_node() {
            if block_pass.expression().is_some_and(|e| e.kind() == NodeKind::SymbolNode) {
                return;
            }
        }
        let span = node.span();
        let text = ctx.text(first.span());
        let replacement: Vec<u8> = text.strip_prefix(b"&").unwrap_or(text).to_vec();
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, replacement)],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}
