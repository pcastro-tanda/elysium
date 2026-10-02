//! `Lint/RefinementImportMethods`, ported from RuboCop's
//! `lib/rubocop/cop/lint/refinement_import_methods.rb`.
//!
//! # Prism shape
//!
//! Whitequark elides a single-statement `do...end` body, so `node.parent`
//! for the sole `include`/`prepend` call inside `refine Foo do ... end` is
//! the `:block` node directly; Prism always wraps a block body in a
//! `StatementsNode`, so this walks a hand-rolled ancestor stack to check
//! that the call is that `StatementsNode`'s only statement, whose own
//! parent is a `BlockNode` attached to a `refine` call.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};

/// Upstream's `MSG` (pre-3.2).
const MSG: &str =
    "Use `import_methods` instead of `{current}` because it is deprecated in Ruby 3.1.";
/// Upstream's `MSG_REMOVED` (3.2+).
const MSG_REMOVED: &str =
    "Use `import_methods` instead of `{current}` because it was removed in Ruby 3.2.";

/// Checks if `include` or `prepend` is called in `refine` block.
#[derive(Debug, Clone)]
pub struct RefinementImportMethods {
    target_ruby_version: f32,
}

impl Rule for RefinementImportMethods {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RefinementImportMethods",
        department: Department::Lint,
        summary:
            "Use `Refinement#import_methods` when using `include` or `prepend` in `refine` block.",
        explanation: "\
These methods are deprecated and should be replaced with `Refinement#import_methods`.

It emulates deprecation warnings in Ruby 3.1. Functionality has been removed in Ruby 3.2.

@safety
  This cop's autocorrection is unsafe because `include M` will affect the included class
  if any changes are made to module `M`.
  On the other hand, `import_methods M` uses a snapshot of method definitions,
  thus it will not be affected if module `M` changes.

```ruby
# bad
refine Foo do
  include Bar
end

# bad
refine Foo do
  prepend Bar
end

# good
refine Foo do
  import_methods Bar
end
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 3.1 {
            return;
        }
        let root = ctx.parsed().root();
        let mut finder = Finder { stack: Vec::new(), candidates: Vec::new() };
        walk(&root, &mut finder);
        let removed = self.target_ruby_version >= 3.2;
        for call in finder.candidates {
            let name = call.name();
            let name = String::from_utf8_lossy(name.as_slice()).into_owned();
            let template = if removed { MSG_REMOVED } else { MSG };
            let message = template.replacen("{current}", &name, 1);
            let Some(selector) = call.message_loc() else { continue };
            let fix = Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(selector.span(), b"import_methods".to_vec())],
            };
            ctx.report_with_fix(&Self::META, selector.span(), message, fix);
        }
    }
}

struct Finder<'pr> {
    stack: Vec<Node<'pr>>,
    candidates: Vec<CallNode<'pr>>,
}

impl<'pr> Visitor<'pr> for Finder<'pr> {
    fn enter(&mut self, node: &Node<'pr>) {
        if let Some(call) = node.as_call_node() {
            if call.receiver().is_none()
                && matches!(call.name().as_slice(), b"include" | b"prepend")
                && in_refine_block(&self.stack)
            {
                self.candidates.push(call);
            }
        }
        self.stack.push(*node);
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

/// Whether the call sits directly in a `refine ... do ... end` block's
/// single-statement body.
fn in_refine_block(stack: &[Node<'_>]) -> bool {
    let Some(statements) = stack.last() else { return false };
    if statements.kind() != NodeKind::StatementsNode {
        return false;
    }
    let Some(stmts) = statements.as_statements_node() else { return false };
    if stmts.body().len() != 1 {
        return false;
    }
    let Some(idx) = stack.len().checked_sub(2) else { return false };
    let Some(block) = stack.get(idx) else { return false };
    let Some(block) = block.as_block_node() else { return false };
    if block.body().map(|b| b.span()) != Some(statements.span()) {
        return false;
    }
    let Some(refine_call_idx) = stack.len().checked_sub(3) else { return false };
    let Some(refine_node) = stack.get(refine_call_idx) else { return false };
    let Some(refine_call) = refine_node.as_call_node() else { return false };
    refine_call.name().as_slice() == b"refine"
        && refine_call.block().is_some_and(|b| b.span() == block.as_node().span())
}
