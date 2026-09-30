//! `Metrics/ModuleLength`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/module_length.rb` plus the `CodeLength` mixin
//! (`lib/rubocop/cop/mixin/code_length.rb`).
//!
//! # `Module.new` assigned to a constant
//!
//! Upstream's `on_casgn` measures `FOO = Module.new do ... end` as a module
//! body:
//!
//! ```ruby
//! def on_casgn(node)
//!   module_definition?(node) { check_code_length(node) }
//! end
//! ```
//!
//! Unlike `Metrics/ClassLength`'s `on_casgn` (which extracts the RHS by hand
//! and passes *that* to `check_code_length`), this passes the *casgn node
//! itself* -- the block only yields `module_definition?`'s capture, which
//! the `check_code_length(node)` call ignores in favour of the closed-over
//! outer `node`. `CodeLength#location` special-cases exactly this: a
//! `casgn`-type node reports at `node.loc.name` (just the constant name,
//! e.g. `Foo`, not the `Module.new do ... end` value), which is why this
//! cop's own `Foo = Module.new do ... end` fixtures underline only `Foo`,
//! unlike `ClassLength`'s equivalent fixtures.
//!
//! One consequence: `module_definition?`'s pattern requires the casgn's
//! third (value) child to itself already be the `(any_block (send
//! (const ...) :new) ...)` shape. A compound assignment (`FOO ||=
//! Module.new do ... end`) or a multiple assignment (`FOO, BAR =
//! Module.new do ... end`) leaves the *casgn*'s own value child empty (the
//! value lives one level up, on the wrapping `or_asgn`/`masgn`), so neither
//! form is ever measured by this cop -- only a plain `FOO = Module.new do
//! ... end` is. This is a real upstream gap (confirmed against 1.91.0), not
//! a porting shortcut, and matches the corpus/fixtures: no compound- or
//! multiple-assignment case exists among them.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util::{constructor_call, CodeLength};

/// Avoid modules longer than 100 lines of code.
#[derive(Debug, Clone)]
pub struct ModuleLength {
    code_length: CodeLength,
}

impl Rule for ModuleLength {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/ModuleLength",
        department: Department::Metrics,
        summary: "Avoid modules longer than 100 lines of code.",
        explanation: "\
Checks if the length of a module exceeds some maximum value. Comment lines \
can optionally be ignored with `CountComments`. Constructs listed in \
`CountAsOne` (`array`, `hash`, `heredoc`, `method_call`) each collapse to a \
single counted line regardless of their own size. This also applies to a \
plain `FOO = Module.new do ... end` constant assignment (reported at the \
constant name, not the `Module.new` value).

```ruby
# bad
Max: 4
module Foo
  def a; end
  def b; end
  def c; end
  def d; end
  def e; end
end

# good
Max: 4
module Foo
  def a; end
  def b; end
end
```",
        enabled_by_default: true,
        severity: Severity::Refactor,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ModuleNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantPathWriteNode,
        ],
        config: &[
            linter::ConfigOption {
                name: "Max",
                default: linter::ConfigDefault::Int(100),
                allowed: &[],
                doc: "Maximum number of counted lines a module may have.",
            },
            linter::ConfigOption {
                name: "CountComments",
                default: linter::ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether full-line comments count towards the total.",
            },
            linter::ConfigOption {
                name: "CountAsOne",
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &["array", "hash", "heredoc", "method_call"],
                doc: "Constructs that count as a single line regardless of their own size.",
            },
        ],
        blind_spots: "\
`FOO ||= Module.new do ... end`/`FOO &&= ...`/`FOO op= ...` and multiple \
assignment (`FOO, BAR = Module.new do ... end`) are never measured, matching \
a real upstream gap (`module_definition?`'s node pattern only ever matches a \
plain `FOO = Module.new do ... end`), not a porting shortcut.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { code_length: CodeLength::from_options(options) })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (value, report_span): (Node<'_>, Span) = match node.kind() {
            NodeKind::ModuleNode => (*node, node.span()),
            NodeKind::ConstantWriteNode => {
                let cw = node.as_constant_write_node().expect("kind matched");
                let value = cw.value();
                if constructor_call(&value, &["Module"]).is_none() {
                    return;
                }
                (value, cw.name_loc().span())
            }
            NodeKind::ConstantPathWriteNode => {
                let cw = node.as_constant_path_write_node().expect("kind matched");
                let value = cw.value();
                if constructor_call(&value, &["Module"]).is_none() {
                    return;
                }
                (value, cw.target().name_loc().span())
            }
            _ => return,
        };

        self.code_length.check(ctx, &Self::META, &value, report_span, "Module");
    }
}
