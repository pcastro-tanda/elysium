//! `Style/MixinUsage`, ported from RuboCop's
//! `lib/rubocop/cop/style/mixin_usage.rb`.
//!
//! # `include_statement` node pattern
//!
//! Upstream's `(send nil? {:include :extend :prepend} const)` fixes the
//! arity at exactly one argument, and that argument must itself be a
//! `:const` node -- whitequark represents both a bare constant (`Foo`) and a
//! namespaced one (`Foo::Bar::Baz`, `::Foo`) as the same `:const` node type
//! (nested for a namespace), so the pattern accepts any of those shapes but
//! rejects zero, two, or more arguments, and rejects a non-constant argument
//! expression. Prism instead gives a bare constant and a `::`-qualified path
//! their own node kinds, [`NodeKind::ConstantReadNode`] and
//! [`NodeKind::ConstantPathNode`]; matching either one, with exactly one
//! argument and no receiver, reproduces the same shape check.
//!
//! # `in_top_level_scope?` node pattern
//!
//! Upstream's recursive pattern walks *up* the parent chain from the send
//! node: it accepts the node outright if it has no parent at all
//! (`root?`), and otherwise requires the parent to be one of `kwbegin`
//! (explicit `begin...end`), `begin` (an implicit multi-statement body --
//! whitequark only synthesizes this wrapper when a body holds more than one
//! statement), `if` (whitequark uses one node type for both `if` and
//! `unless`), or `def`, recursing on that parent. Any other parent type --
//! notably `class`, `module`, `sclass`, or any block -- stops the walk and
//! fails the match.
//!
//! Prism has no single-statement special case: every statement sequence
//! (a top-level program body, a `def`/`if`/`unless`/`begin` body, a class,
//! module, or block body) is always wrapped in its own
//! [`NodeKind::StatementsNode`], and an `if`/`unless`'s `else` branch is
//! further wrapped in its own [`NodeKind::ElseNode`]. Neither wrapper
//! represents a new scope, so both are transparent here exactly like
//! whitequark's implicit `begin`; [`NodeKind::BeginNode`] (Prism's explicit
//! `begin...end`), [`NodeKind::IfNode`], [`NodeKind::UnlessNode`], and
//! [`NodeKind::DefNode`] mirror `kwbegin`/`if`/`def` and, like them, only
//! keep the walk alive rather than ending it outright. The walk succeeds
//! once it reaches [`NodeKind::ProgramNode`] (the true root, matching
//! `root?`) and fails the instant it meets anything else, in particular
//! [`NodeKind::ClassNode`], [`NodeKind::ModuleNode`], and
//! [`NodeKind::BlockNode`].
//!
//! [`linter::Context::ancestors`] already carries exactly the kinds this
//! walk needs (outermost first, i.e. the true root is always its first
//! element and the immediate parent is always its last), so no
//! self-maintained ancestor stack is needed here.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// `Style::MixinUsage`.
#[derive(Debug, Clone)]
pub struct MixinUsage;

impl Rule for MixinUsage {
    const META: RuleMeta = RuleMeta {
        name: "Style/MixinUsage",
        department: Department::Style,
        summary: "Checks that `include`, `extend` and `prepend` statements appear inside \
                  classes and modules, not at the top level.",
        explanation: "\
Checks that `include`, `extend` and `prepend` statements appear
inside classes and modules, not at the top level, so as to not affect
the behavior of `Object`.

```ruby
# bad
include M

class C
end

# bad
extend M

class C
end

# bad
prepend M

class C
end

# good
class C
  include M
end

# good
class C
  extend M
end

# good
class C
  prepend M
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
Only a single constant argument is recognized (`include M`, `include M1::M2::M3`), matching
upstream's fixed-arity `(send nil? {:include :extend :prepend} const)` pattern: `include M1, M2`
(two mixins in one call) never matches upstream's pattern either, and is not flagged here.",
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
        if !matches!(name, b"include" | b"extend" | b"prepend") {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let args = args.arguments();
        if args.len() != 1 {
            return;
        }
        let arg = args.first().expect("len checked above");
        if !matches!(arg.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode) {
            return;
        }
        if !in_top_level_scope(ctx) {
            return;
        }
        let statement = String::from_utf8_lossy(name);
        ctx.report(
            &Self::META,
            node.span(),
            format!("`{statement}` is used at the top level. Use inside `class` or `module`."),
        );
    }
}

/// RuboCop's `in_top_level_scope?`, applied to the node currently being
/// entered via [`Context::ancestors`] (outermost first): walks from the
/// immediate parent outward, passing transparently through
/// [`NodeKind::StatementsNode`] and [`NodeKind::ElseNode`] (implicit
/// sequencing, no scope of their own) and [`NodeKind::BeginNode`],
/// [`NodeKind::IfNode`], [`NodeKind::UnlessNode`], and [`NodeKind::DefNode`]
/// (upstream's `kwbegin`/`begin`/`if`/`def`), succeeding at
/// [`NodeKind::ProgramNode`] (the true root) and failing at anything else.
fn in_top_level_scope(ctx: &Context<'_>) -> bool {
    for info in ctx.ancestors().iter().rev() {
        match info.kind {
            NodeKind::ProgramNode => return true,
            NodeKind::StatementsNode
            | NodeKind::ElseNode
            | NodeKind::BeginNode
            | NodeKind::IfNode
            | NodeKind::UnlessNode
            | NodeKind::DefNode => {}
            _ => return false,
        }
    }
    false
}
