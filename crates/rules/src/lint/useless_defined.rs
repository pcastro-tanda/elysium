//! `Lint/UselessDefined`, ported from RuboCop's
//! `lib/rubocop/cop/lint/useless_defined.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Checks for calls to `defined?` with strings and symbols. The result of
/// such a call will always be truthy.
#[derive(Debug, Clone)]
pub struct UselessDefined;

impl Rule for UselessDefined {
    const META: RuleMeta = RuleMeta {
        name: "Lint/UselessDefined",
        department: Department::Lint,
        summary: "Checks for calls to `defined?` with strings and symbols. The result of such a call will always be truthy.",
        explanation: "\
Checks for calls to `defined?` with strings or symbols as the argument.
Such calls will always return `'expression'`, you probably meant to check
for the existence of a constant, method, or variable instead.

`defined?` is part of the Ruby syntax and doesn't behave like normal
methods. You can safely pass in what you are checking for directly,
without encountering a `NameError`.

When interpolation is used, oftentimes it is not possible to write the
code with `defined?`. In these cases, switch to one of the more specific
methods:

* `class_variable_defined?`
* `const_defined?`
* `method_defined?`
* `instance_variable_defined?`
* `binding.local_variable_defined?`

```ruby
# bad
defined?('FooBar')
defined?(:FooBar)
defined?(:foo_bar)
defined?('foo_bar')

# good
defined?(FooBar)
defined?(foo_bar)

# bad - interpolation
bar = 'Bar'
defined?(\"Foo::#{bar}::Baz\")

# good
bar = 'Bar'
defined?(Foo) && Foo.const_defined?(bar) && Foo.const_get(bar).const_defined?(:Baz)
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefinedNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(defined) = node.as_defined_node() else { return };
        let value = defined.value();
        let ty = match value.kind() {
            NodeKind::StringNode | NodeKind::InterpolatedStringNode => "string",
            NodeKind::SymbolNode | NodeKind::InterpolatedSymbolNode => "symbol",
            _ => return,
        };
        let message =
            format!("Calling `defined?` with a {ty} argument will always return a truthy value.");
        ctx.report(&Self::META, node.span(), message);
    }
}
