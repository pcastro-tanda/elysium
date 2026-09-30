//! `Layout/EmptyLinesAroundMethodBody`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_lines_around_method_body.rb` plus the
//! `EmptyLinesAroundBody` mixin it includes (shared logic lives in
//! `empty_lines_around_body.rs`). `EnforcedStyle` is hardcoded to
//! `no_empty_lines`, with no config; the cop fires for both `def` and `defs`
//! (a receiver-qualified `def self.foo`), which Prism represents with the
//! same [`NodeKind::DefNode`] (RuboCop's `alias on_defs on_def`).
//!
//! An endless method (`def foo = expr`) has no `end` keyword and its body is
//! the expression itself, not a block that can hold a leading/trailing blank
//! line the usual way -- RuboCop special-cases it
//! (`offending_endless_method?`/`register_offense_for_endless_method`) to
//! only ever check for an *extra* blank line directly after the `=`, and
//! only when the body starts more than one line past it (a single-line
//! endless method has no following physical line to even check); this port
//! replicates that guard explicitly, then calls the shared
//! `check_beginning` for the actual blank-line check.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

use super::empty_lines_around_body::{check_beginning, check_both, Want};

/// Keeps track of empty lines around method bodies.
#[derive(Debug, Clone)]
pub struct EmptyLinesAroundMethodBody;

impl Rule for EmptyLinesAroundMethodBody {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLinesAroundMethodBody",
        department: Department::Layout,
        summary: "Keeps track of empty lines around method bodies.",
        explanation: "\
```ruby
# good

def foo
  # ...
end

# bad

def bar

  # ...

end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");

        if let Some(equal_loc) = def.equal_loc() {
            // RuboCop's `offending_endless_method?`: only when the body
            // starts more than one line past the `=` (i.e. there's room
            // for a blank line to actually exist between them) is the
            // line right after the `=` even checked -- guarding against a
            // single-line endless method, where the body sits on the very
            // same line as the `=` and there is no following line at all.
            let assign_line = ctx.line_col(equal_loc.span().start).line;
            if let Some(body) = def.body() {
                let body_first_line = ctx.line_col(body.span().start).line;
                if body_first_line > assign_line + 1 {
                    check_beginning(ctx, &Self::META, "method", Want::NoEmpty, assign_line);
                }
            }
            return;
        }

        let span = node.span();
        if ctx.is_single_line(span) {
            return;
        }

        // Whitequark's `args` node source range spans the whole `(...)`
        // delimiter pair when parens are present, even for an empty
        // argument list -- not just the parameter tokens themselves -- so
        // the closing paren's own line takes priority over the last
        // parameter's line whenever parens exist.
        let first_line = if let Some(rparen) = def.rparen_loc() {
            Some(ctx.line_col(rparen.span().start).line)
        } else {
            def.parameters().map(|p| ctx.last_line(p.location().span()))
        };
        let first_line = first_line.unwrap_or_else(|| ctx.line_col(span.start).line);
        let last_line = ctx.last_line(span);
        check_both(ctx, &Self::META, "method", Want::NoEmpty, first_line, last_line);
    }
}
