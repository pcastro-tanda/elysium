//! `Metrics/ClassLength`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/class_length.rb` plus the `CodeLength` mixin
//! (`lib/rubocop/cop/mixin/code_length.rb`).
//!
//! # `class << self`
//!
//! Upstream's `on_sclass` re-runs `on_class`'s check, but only for a
//! singleton class with no enclosing `class` ancestor -- one nested inside a
//! `class` is already measured as part of that class's own body (Prism's
//! `SingletonClassNode` is not `is_classlike` in `metrics/util.rs`, so its
//! lines are never excluded the way an inner `class`/`module`'s are), and
//! double-counting it as its own top-level "Class" offense would be wrong.
//! A *top-level* `class << self` (no enclosing class at all) has no other
//! way to be measured, so it is checked as if it were its own class.
//!
//! # `Struct.new`/`Class.new` assigned to a constant
//!
//! Upstream's `on_casgn` measures `FOO = Struct.new(...) do ... end` (and
//! `Class.new`, but not `Module.new`, which `Metrics/ModuleLength` covers
//! instead) as a class body, reporting on the RHS rather than the whole
//! assignment. Prism gives every constant-assignment shape (plain, `A::FOO`,
//! `||=`/`&&=`/`op=` in both forms, and multiple assignment) its own flat
//! node exposing the assigned value directly (see `util::assigned_value`),
//! unlike whitequark, where a compound or multiple assignment nests a
//! valueless `casgn` reached only by walking back up through its parent(s).

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::util::{assigned_value, constructor_call, CodeLength};

/// Avoid classes longer than 100 lines of code.
#[derive(Debug, Clone)]
pub struct ClassLength {
    code_length: CodeLength,
}

const KINDS: &[NodeKind] = &[
    NodeKind::ClassNode,
    NodeKind::SingletonClassNode,
    NodeKind::ConstantWriteNode,
    NodeKind::ConstantPathWriteNode,
    NodeKind::ConstantOrWriteNode,
    NodeKind::ConstantAndWriteNode,
    NodeKind::ConstantOperatorWriteNode,
    NodeKind::ConstantPathOrWriteNode,
    NodeKind::ConstantPathAndWriteNode,
    NodeKind::ConstantPathOperatorWriteNode,
    NodeKind::MultiWriteNode,
];

impl Rule for ClassLength {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/ClassLength",
        department: Department::Metrics,
        summary: "Avoid classes longer than 100 lines of code.",
        explanation: "\
Checks if the length of a class exceeds some maximum value. Comment lines can \
optionally be ignored with `CountComments`. Constructs listed in `CountAsOne` \
(`array`, `hash`, `heredoc`, `method_call`) each collapse to a single counted \
line regardless of their own size. This also applies to `Struct.new`/\
`Class.new` definitions, including ones assigned to a constant, and to a \
top-level `class << self`.

```ruby
# bad
Max: 4
class Foo
  def a; end
  def b; end
  def c; end
  def d; end
  def e; end
end

# good
Max: 4
class Foo
  def a; end
  def b; end
end
```",
        enabled_by_default: true,
        severity: Severity::Refactor,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: KINDS,
        config: &[
            linter::ConfigOption {
                name: "Max",
                default: linter::ConfigDefault::Int(100),
                allowed: &[],
                doc: "Maximum number of counted lines a class may have.",
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
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { code_length: CodeLength::from_options(options) })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let target = match node.kind() {
            NodeKind::ClassNode => *node,
            NodeKind::SingletonClassNode => {
                if ctx.ancestors().iter().any(|a| a.kind == NodeKind::ClassNode) {
                    return;
                }
                *node
            }
            _ => {
                let Some(value) = assigned_value(node) else { return };
                let Some(call) = constructor_call(&value, &["Struct", "Class"]) else { return };
                call.as_node()
            }
        };

        self.code_length.check(ctx, &Self::META, &target, target.span(), "Class");
    }
}
