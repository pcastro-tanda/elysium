//! `Gemspec/AddRuntimeDependency`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/add_runtime_dependency.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Use `add_dependency` instead of `add_runtime_dependency`.";

/// Prefer `add_dependency` over `add_runtime_dependency`.
#[derive(Debug, Clone)]
pub struct AddRuntimeDependency;

impl Rule for AddRuntimeDependency {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/AddRuntimeDependency",
        department: Department::Gemspec,
        summary: "Prefer `add_dependency` over `add_runtime_dependency`.",
        explanation: "\
Prefer `add_dependency` over `add_runtime_dependency` as the latter is
considered soft-deprecated.

```ruby
# bad
Gem::Specification.new do |spec|
  spec.add_runtime_dependency('rubocop')
end

# good
Gem::Specification.new do |spec|
  spec.add_dependency('rubocop')
end
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"add_runtime_dependency" {
            return;
        }
        if call.receiver().is_none() {
            return;
        }
        let Some(args) = call.arguments() else { return };
        if args.arguments().is_empty() {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        let span = selector.span();
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, b"add_dependency".to_vec())],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}
