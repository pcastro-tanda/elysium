//! `Lint/UselessElseWithoutRescue`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_else_without_rescue.rb`.
//!
//! # Unreachable in this engine
//!
//! Upstream detects this via `processed_source.diagnostics` for a
//! `:useless_else` reason emitted by its own (whitequark) parser: on Ruby
//! 2.6+ `begin ... else ... end` without a `rescue` is not merely
//! discouraged, it is a parse error (`maximum_target_ruby_version 2.5`
//! reflects exactly that -- the cop only makes sense pre-2.6). Prism agrees:
//! it reports this construct as a syntax error too, and this engine's
//! `lint_parsed_with` short-circuits on any Prism parse error, emitting only
//! `Lint/Syntax` and skipping the node walk entirely (`node_count: 0`), so
//! no `Rule::enter` ever runs against the offending source. The single
//! fixture case here (`else_with_rescue_accepts.rb`) exercises the only
//! reachable path: a `begin/rescue/else/end` that parses cleanly and must
//! not be flagged. The `enter` below mirrors upstream's condition literally
//! for documentation purposes even though it is unreachable in practice.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "`else` without `rescue` is useless.";

/// Checks for useless `else` in `begin..end` without `rescue`.
#[derive(Debug, Clone)]
pub struct UselessElseWithoutRescue {
    target_ruby_version: f32,
}

impl Rule for UselessElseWithoutRescue {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessElseWithoutRescue",
        department: Department::Lint,
        summary: "Checks for useless `else` in `begin..end` without `rescue`.",
        explanation: "\
`begin`/`end` blocks that have an `else` but no `rescue` are useless: the \
`else` branch runs unconditionally, exactly like the code preceding it \
would, since it only ever executes when no exception was raised. This is \
not valid syntax on Ruby 2.6 or higher.\n\n\
```ruby\n\
# bad\n\
begin\n\
  do_something\n\
else\n\
  do_something_else # This will never be run.\n\
end\n\n\
# good\n\
begin\n\
  do_something\n\
rescue\n\
  handle_errors\n\
else\n\
  do_something_else\n\
end\n\
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::BeginNode],
        config: &[],
        blind_spots: "\
Upstream fires from its own parser's `:useless_else` diagnostic, which \
covers `begin/else/end` without `rescue` written anywhere a `begin` can \
appear (including implicit method/block bodies). On Ruby 2.6+ this \
construct is a syntax error under Prism too, and this engine reports only \
`Lint/Syntax` and skips the node walk entirely for any file with a parse \
error, so this rule can never actually fire in practice.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version > 2.5 {
            return;
        }
        let Some(begin) = node.as_begin_node() else { return };
        let Some(else_clause) = begin.else_clause() else { return };
        if begin.rescue_clause().is_some() {
            return;
        }
        ctx.report(&Self::META, else_clause.else_keyword_loc().span(), MSG);
    }
}
