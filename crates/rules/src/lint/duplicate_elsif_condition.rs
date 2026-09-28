//! `Lint/DuplicateElsifCondition`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_elsif_condition.rb`.
//!
//! # Node shapes
//!
//! Whitequark's parser has no distinct node type for `elsif`: an entire
//! `if`/`elsif`/`else` chain is one nested `(if cond if_branch else_branch)`
//! tree, where `else_branch` is either another `(if ...)` node (the next
//! `elsif`), the real `else` body, or `nil`. Prism instead gives `elsif` its
//! own [`ruby_ast::node::IfNode`] (distinguished from a genuine, separately
//! `end`-terminated `if` only by [`IfNode::if_keyword_loc`] reading
//! `"elsif"` rather than `"if"`), reached through the parent's
//! [`IfNode::subsequent`] -- which, for a real trailing `else`, instead
//! holds an [`ruby_ast::node::ElseNode`] wrapper. [`check_chain`] walks this
//! link exactly like RuboCop's `while node.if? || node.elsif?` loop, just
//! stopping (rather than unwrapping into) a trailing `ElseNode`, since
//! nothing past it is ever a condition to compare.
//!
//! # Why only the chain's head walks
//!
//! RuboCop's `on_if` callback fires for every `:if`-type node the AST
//! visitor reaches, including each nested `elsif` link (it is a node in the
//! tree too) -- but `add_offense` is deduplicated per unique range within a
//! single cop run (`Base#add_offense`'s `current_offense_locations.add?`),
//! so re-walking from a link, which only ever *re-finds a subset* of what
//! walking from the chain's true head already found (any repeat visible
//! starting partway through the chain was, by definition, already visible
//! starting from the head, since the head's `previous` list is a superset
//! of the link's), never adds a new offense. This port skips that redundant
//! subset work entirely by only walking from [`IfNode`]s whose
//! `if_keyword_loc` reads `"if"` (the genuine chain head, whether at the
//! top level or nested inside an unrelated branch) rather than maintaining
//! an explicit dedup set, which is observationally identical.
//!
//! # Condition equality
//!
//! `previous.include?(condition)` is whitequark/parser's generic
//! `Parser::AST::Node#==` (structural, type-and-children, ignoring source
//! position). This port approximates it with exact source-text comparison
//! instead, matching this codebase's established approximation for the
//! same problem in `Style/RedundantCondition`/`Lint/SelfAssignment`: every
//! fixture condition is written identically both times it repeats, so this
//! is exact for everything tested; a genuinely equal condition written with
//! different incidental formatting (extra parens, different whitespace)
//! would be treated as unequal.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::IfNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Duplicate `elsif` condition detected.";

/// Checks that there are no repeated conditions used in `if`/`elsif`.
#[derive(Debug, Clone, Default)]
pub struct DuplicateElsifCondition;

impl Rule for DuplicateElsifCondition {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateElsifCondition",
        department: Department::Lint,
        summary: "Checks that there are no repeated conditions used in if 'elsif'.",
        explanation: "\
Checks that there are no repeated conditions used in if 'elsif'.

```ruby
# bad
if x == 1
  do_something
elsif x == 1
  do_something_else
end

# good
if x == 1
  do_something
elsif x == 2
  do_something_else
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::IfNode],
        config: &[],
        blind_spots: "\
Conditions are compared by exact source text rather than RuboCop's true
structural `Node#==`: a genuinely equal condition written with different
incidental formatting (extra parens, different whitespace) is treated as
unequal.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Node::IfNode { .. } = node else { return };
        let head = node.as_if_node().expect("kind matched");
        // Ternaries have no `if`/`elsif` keyword at all; a genuine `elsif`
        // link is walked as part of its chain's head, not independently
        // (see the module doc).
        let Some(kw) = head.if_keyword_loc() else { return };
        if kw.as_slice() != b"if" {
            return;
        }
        check_chain(&head, ctx);
    }
}

/// RuboCop's `on_if` loop: walks the `if`/`elsif` chain starting at `head`,
/// reporting each condition whose source text repeats an earlier one in
/// the same chain.
fn check_chain(head: &IfNode<'_>, ctx: &mut Context<'_>) {
    let mut previous: Vec<&[u8]> = Vec::new();
    let mut current = head.as_node();
    while let Some(if_node) = current.as_if_node() {
        let condition = if_node.predicate();
        let text = ctx.text(condition.span());
        if previous.contains(&text) {
            ctx.report(&DuplicateElsifCondition::META, condition.span(), MSG);
        }
        previous.push(text);
        // RuboCop's `node = node.else_branch; break unless node&.if_type?`:
        // `subsequent` is `None` (no `else`/`elsif`), an `elsif` link
        // (another `IfNode`, continue), or a real `ElseNode` (stop; its
        // body holds no more conditions to compare).
        let Some(next) = if_node.subsequent() else { break };
        if !matches!(next, Node::IfNode { .. }) {
            break;
        }
        current = next;
    }
}
