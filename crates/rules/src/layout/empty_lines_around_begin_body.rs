//! `Layout/EmptyLinesAroundBeginBody`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_begin_body.rb` plus the
//! `EmptyLinesAroundBody` mixin it includes (shared logic lives in
//! `empty_lines_around_body.rs`).
//!
//! Unlike the class/module/method/block cops, this one never inspects the
//! body's own shape: `check(node, nil)` upstream passes a `nil` body, so only
//! the blank line right after `begin` and right before `end` are ever
//! checked (`EnforcedStyle` is hardcoded to `no_empty_lines`, with no
//! config).
//!
//! Prism represents both an explicit `begin...end` keyword block and the
//! implicit body a `def`/block gets when it has a `rescue`/`ensure` clause
//! with the very same [`NodeKind::BeginNode`]; only the former has a real
//! `begin_keyword_loc`, which is how this cop tells them apart (the latter is
//! `Layout/EmptyLinesAroundMethodBody`'s and `Layout/EmptyLinesAroundBlockBody`'s
//! job instead).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::empty_lines_around_body::{check_both, Want};

/// Keeps track of empty lines around begin-end bodies.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundBeginBody;

impl Rule for EmptyLinesAroundBeginBody {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundBeginBody",
        department: Department::Layout,
        summary: "Keeps track of empty lines around begin-end bodies.",
        explanation: "\
```ruby
# bad
begin

  # ...

end

# good
begin
  # ...
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::BeginNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let begin = node.as_begin_node().expect("kind matched");
        // Prism uses the same `BeginNode` for an implicit `rescue`/`ensure`
        // wrapper around a `def`/block body; only a real `begin ... end`
        // keyword construct has its own `begin_keyword_loc`.
        if begin.begin_keyword_loc().is_none() {
            return;
        }
        let span = node.span();
        if ctx.is_single_line(span) {
            return;
        }
        let first_line = ctx.line_col(span.start).line;
        let last_line = ctx.last_line(span);
        check_both(ctx, &Self::META, "`begin`", Want::NoEmpty, first_line, last_line);
    }
}
