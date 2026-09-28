//! `Lint/EnsureReturn`, ported from RuboCop's
//! `lib/rubocop/cop/lint/ensure_return.rb`.
//!
//! Upstream's `on_ensure` reads the `ensure` clause's `branch` (its body)
//! and walks it with `each_node(:return)`, which -- like rubocop-ast's
//! `each_descendant` -- does not respect scope boundaries: a `return`
//! nested inside a `def`, block, or lambda written *inside* the `ensure`
//! body is still visited and flagged. [`ruby_ast::each_descendant`] has the
//! identical "scope boundaries are not respected" behaviour (see its own
//! doc comment), so subscribing to [`NodeKind::EnsureNode`] and walking its
//! `statements()` with that helper reproduces `on_ensure` exactly, blind
//! spot included.
//!
//! Prism's `EnsureNode::statements` is `None` for an empty `ensure` body
//! (`begin; foo; ensure; end`), matching whitequark's `branch` being `nil`
//! in the same case -- both simply skip the walk.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Do not return from an `ensure` block.";

/// Checks for `return` from an `ensure` block.
#[derive(Debug, Clone, Default)]
pub struct EnsureReturn;

impl Rule for EnsureReturn {
    const META: RuleMeta = RuleMeta {
        name: "Lint/EnsureReturn",
        department: Department::Lint,
        summary: "Checks for `return` from an `ensure` block.",
        explanation: "\
Checks for `return` from an `ensure` block. `return` from an ensure block is a dangerous code
smell as it will take precedence over any exception being raised, and the exception will be
silently thrown away as if it were rescued.

If you want to rescue some (or all) exceptions, best to do it explicitly.

```ruby
# bad
def foo
  do_something
ensure
  cleanup
  return self
end

# good
def foo
  do_something
  self
ensure
  cleanup
end

# good
def foo
  begin
    do_something
  rescue SomeException
    # Let's ignore this exception
  end
  self
ensure
  cleanup
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::EnsureNode],
        config: &[],
        blind_spots: "\
Matches upstream's own blind spot: `each_node`/`each_descendant` do not respect scope boundaries,
so a `return` nested inside a `def`, block, or lambda written inside the `ensure` body is still
flagged even though it returns from that inner scope, not from the method the `ensure` belongs to.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(ensure) = node.as_ensure_node() else { return };
        let Some(statements) = ensure.statements() else { return };
        let statements = statements.as_node();
        each_descendant(&statements, &mut |descendant| {
            if descendant.kind() == NodeKind::ReturnNode {
                ctx.report(&Self::META, descendant.span(), MSG);
            }
        });
    }
}
