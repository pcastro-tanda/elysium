//! `Style/ClassVars`, ported from RuboCop's
//! `lib/rubocop/cop/style/class_vars.rb`.
//!
//! Offenses are signaled only on *assignment* to a class variable, never on
//! a read: upstream's `on_cvasgn` fires for `@@test = 10` and `on_send`
//! (restricted to `class_variable_set`) fires for `class_variable_set(:@@test,
//! ...)`/`obj.class_variable_set(:@@test, ...)` regardless of receiver.
//!
//! Whitequark's `cvasgn` node is not limited to the plain-assignment shape:
//! it is also the node type used for the inner target of `@@test ||= 1`
//! (`or-asgn`), `@@test &&= 1` (`and-asgn`), `@@test += 1` (`op-asgn`), and
//! each element of a multiple-assignment's left-hand side (`@@a, @@b = 1,
//! 2`), so `on_cvasgn` fires for all of those shapes too. Prism splits
//! these into their own node kinds --
//! [`NodeKind::ClassVariableOrWriteNode`],
//! [`NodeKind::ClassVariableAndWriteNode`],
//! [`NodeKind::ClassVariableOperatorWriteNode`], and
//! [`NodeKind::ClassVariableTargetNode`] -- so this port subscribes to all
//! of them (alongside the plain [`NodeKind::ClassVariableWriteNode`]) to
//! match upstream's exact coverage.
//!
//! The message differs by shape because upstream formats a different
//! source for each: `on_cvasgn` uses `node.children.first`, the plain
//! variable-name symbol (`Symbol#to_s` strips the leading colon, e.g.
//! `@@test`), while `on_send` uses `first_argument.source`, the argument's
//! raw source text (keeping a symbol literal's leading colon, e.g.
//! `:@@test`, or a string literal's quotes). This port reproduces both
//! verbatim: the write node's `name()` for the assignment shapes, and
//! `ctx.text(first_argument.span())` for the call shape.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// Checks for uses of class variables.
#[derive(Debug, Clone, Default)]
pub struct ClassVars;

impl ClassVars {
    /// RuboCop's `on_send`, restricted to `RESTRICT_ON_SEND = %i[class_variable_set]`.
    fn check_call(ctx: &mut Context<'_>, call: &CallNode<'_>) {
        if call.name().as_slice() != b"class_variable_set" {
            return;
        }
        let Some(first_argument) = call.arguments().and_then(|args| args.arguments().iter().next())
        else {
            return;
        };
        let span = first_argument.span();
        let class_var = String::from_utf8_lossy(ctx.text(span)).into_owned();
        ctx.report(
            &Self::META,
            span,
            format!("Replace class var {class_var} with a class instance var."),
        );
    }

    /// RuboCop's `on_cvasgn`: `node.loc.name` with the plain variable-name
    /// symbol, for every Prism node kind whitequark's `cvasgn` maps onto.
    fn report(ctx: &mut Context<'_>, span: ruby_source::Span) {
        let class_var = String::from_utf8_lossy(ctx.text(span)).into_owned();
        ctx.report(
            &Self::META,
            span,
            format!("Replace class var {class_var} with a class instance var."),
        );
    }
}

impl Rule for ClassVars {
    const META: RuleMeta = RuleMeta {
        name: "Style/ClassVars",
        department: Department::Style,
        summary: "Checks for uses of class variables.",
        explanation: "\
Checks for uses of class variables. Offenses
are signaled only on assignment to class variables to
reduce the number of offenses that would be reported.

You have to be careful when setting a value for a class
variable; if a class has been inherited, changing the
value of a class variable also affects the inheriting
classes. This means that it's almost always better to
use a class instance variable instead.

```ruby
# bad
class A
  @@test = 10
end

class A
  def self.test(name, value)
    class_variable_set(\"@@#{name}\", value)
  end
end

class A; end
A.class_variable_set(:@@test, 10)

# good
class A
  @test = 10
end

class A
  def test
    @@test # you can access class variable without offense
  end
end

class A
  def self.test(name)
    class_variable_get(\"@@#{name}\") # you can access without offense
  end
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ClassVariableWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::ClassVariableTargetNode,
            NodeKind::CallNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ClassVariableWriteNode => {
                if let Some(w) = node.as_class_variable_write_node() {
                    Self::report(ctx, w.name_loc().span());
                }
            }
            NodeKind::ClassVariableOrWriteNode => {
                if let Some(w) = node.as_class_variable_or_write_node() {
                    Self::report(ctx, w.name_loc().span());
                }
            }
            NodeKind::ClassVariableAndWriteNode => {
                if let Some(w) = node.as_class_variable_and_write_node() {
                    Self::report(ctx, w.name_loc().span());
                }
            }
            NodeKind::ClassVariableOperatorWriteNode => {
                if let Some(w) = node.as_class_variable_operator_write_node() {
                    Self::report(ctx, w.name_loc().span());
                }
            }
            NodeKind::ClassVariableTargetNode => {
                if node.as_class_variable_target_node().is_some() {
                    Self::report(ctx, node.span());
                }
            }
            NodeKind::CallNode => {
                if let Some(call) = node.as_call_node() {
                    Self::check_call(ctx, &call);
                }
            }
            _ => {}
        }
    }
}
