//! `Lint/CircularArgumentReference`, ported from RuboCop's
//! `lib/rubocop/cop/lint/circular_argument_reference.rb`.
//!
//! Upstream subscribes to whitequark's `kwoptarg`/`optarg` node types (an
//! optional keyword argument and an optional ordinal argument, respectively);
//! Prism's equivalents are [`NodeKind::OptionalKeywordParameterNode`] and
//! [`NodeKind::OptionalParameterNode`], each exposing the argument's `name`
//! and its default-value expression directly (no destructuring needed, unlike
//! whitequark's `*node` children unpacking).
//!
//! `check_assignment_chain` walks a chain of whitequark `lvasgn` nodes
//! (`current_node.children.last` steps into the assigned value, repeating
//! while that value is itself an assignment) to find the innermost read and
//! the set of names assigned along the way. Prism represents each `x = y` in
//! that chain as a [`NodeKind::LocalVariableWriteNode`] whose
//! [`ruby_ast::node::LocalVariableWriteNode::value`] is the next node in the
//! chain, so [`check_assignment_chain`] walks that accessor instead of a
//! generic `children` array.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::ConstantId;
use ruby_ast::{Node, NodeExt, NodeKind};
use std::collections::HashSet;

/// RuboCop's `MSG`.
const MSG: &str = "Circular argument reference - `{}`.";

/// Checks for circular argument references in optional keyword arguments and
/// optional ordinal arguments.
#[derive(Debug, Clone)]
pub struct CircularArgumentReference;

impl Rule for CircularArgumentReference {
    const META: RuleMeta = RuleMeta {
        name: "Lint/CircularArgumentReference",
        department: Department::Lint,
        summary: "Checks for circular argument references in optional keyword arguments and \
                   optional ordinal arguments.",
        explanation: "\
Checks for circular argument references in optional keyword
arguments and optional ordinal arguments.

NOTE: This syntax was made invalid on Ruby 2.7 - Ruby 3.3 but is allowed
again since Ruby 3.4.

```ruby
# bad

def bake(pie: pie)
  pie.heat_up
end

# good

def bake(pie:)
  pie.refrigerate
end

# good

def bake(pie: self.pie)
  pie.feed_to(user)
end

# bad

def cook(dry_ingredients = dry_ingredients)
  dry_ingredients.reduce(&:+)
end

# good

def cook(dry_ingredients = self.dry_ingredients)
  dry_ingredients.combine
end

# bad

def foo(pie = pie = pie)
  pie.heat_up
end

# good

def foo(pie)
  pie.heat_up
end

# bad

def foo(pie = cake = pie)
  [pie, cake].each(&:heat_up)
end

# good

def foo(cake = pie)
  [pie, cake].each(&:heat_up)
end
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::OptionalParameterNode, NodeKind::OptionalKeywordParameterNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let (arg_name, arg_value) = match node.kind() {
            NodeKind::OptionalParameterNode => {
                let n = node.as_optional_parameter_node().expect("kind matched");
                (n.name(), n.value())
            }
            NodeKind::OptionalKeywordParameterNode => {
                let n = node.as_optional_keyword_parameter_node().expect("kind matched");
                (n.name(), n.value())
            }
            _ => return,
        };

        check_for_circular_argument_reference(ctx, &arg_name, &arg_value);
    }
}

/// RuboCop's `check_for_circular_argument_references`.
fn check_for_circular_argument_reference(
    ctx: &mut Context<'_>,
    arg_name: &ConstantId<'_>,
    arg_value: &Node<'_>,
) {
    if let Some(read) = arg_value.as_local_variable_read_node() {
        if read.name().as_slice() == arg_name.as_slice() {
            report(ctx, arg_name, arg_value);
            return;
        }
    }

    check_assignment_chain(ctx, arg_name, arg_value);
}

/// RuboCop's `check_assignment_chain`: walks a chain of local-variable
/// assignments (`current_node = current_node.value` while `current_node` is
/// itself an assignment), collecting every assigned name along the way, then
/// checks whether the chain bottoms out at a read of one of those names or of
/// the original argument name.
fn check_assignment_chain(ctx: &mut Context<'_>, arg_name: &ConstantId<'_>, node: &Node<'_>) {
    let mut seen_variables: HashSet<&[u8]> = HashSet::new();
    let mut current = *node;

    while let Some(write) = current.as_local_variable_write_node() {
        seen_variables.insert(write.name().as_slice());
        current = write.value();
    }

    let Some(read) = current.as_local_variable_read_node() else {
        return;
    };
    let variable_name = read.name().as_slice();
    if seen_variables.contains(variable_name) || variable_name == arg_name.as_slice() {
        report(ctx, arg_name, &current);
    }
}

/// RuboCop's `add_offense(node, message: format(MSG, arg_name: arg_name))`.
fn report(ctx: &mut Context<'_>, arg_name: &ConstantId<'_>, node: &Node<'_>) {
    let name = String::from_utf8_lossy(arg_name.as_slice());
    let message = MSG.replacen("{}", &name, 1);
    ctx.report(&CircularArgumentReference::META, node.span(), message);
}
