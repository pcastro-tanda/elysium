//! `Security/YAMLLoad`, ported from RuboCop's
//! `lib/rubocop/cop/security/yaml_load.rb`.
//!
//! # `maximum_target_ruby_version 3.0`
//!
//! Upstream's `TargetRubyVersion` mixin disables this cop entirely above
//! Ruby 3.0: Ruby 3.1+ (Psych 4) makes `YAML.load` behave like
//! `YAML.safe_load` by default, so there is nothing to flag.
//! [`YAMLLoad::enter`] mirrors this with a plain
//! `target_ruby_version > 3.0` guard rather than not subscribing at all
//! (there is no static "only run below version X" hook in this engine).
//! Fixtures default to target Ruby 3.3, so every fixture case is a
//! "does not register an offense" case; this is expected, not a bug.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Prefer using `YAML.safe_load` over `YAML.load`.";

/// Checks for the use of YAML class methods which have potential security
/// issues leading to remote code execution when loading from an untrusted
/// source.
#[derive(Debug, Clone)]
pub struct YAMLLoad {
    target_ruby_version: f32,
}

impl Rule for YAMLLoad {
    const META: RuleMeta = RuleMeta {
        name: "Security/YAMLLoad",
        department: Department::Security,
        summary: "Prefer usage of `YAML.safe_load` over `YAML.load` due to potential security \
                   issues. See reference for more information.",
        explanation: "\
Checks for the use of YAML class methods which have
potential security issues leading to remote code execution when
loading from an untrusted source.

NOTE: Ruby 3.1+ (Psych 4) uses `Psych.load` as `Psych.safe_load` by default.

```ruby
# bad
YAML.load(\"--- !ruby/object:Foo {}\") # Psych 3 is unsafe by default

# good
YAML.safe_load(\"--- !ruby/object:Foo {}\", [Foo])                    # Ruby 2.5  (Psych 3)
YAML.safe_load(\"--- !ruby/object:Foo {}\", permitted_classes: [Foo]) # Ruby 3.0- (Psych 3)
YAML.load(\"--- !ruby/object:Foo {}\", permitted_classes: [Foo])      # Ruby 3.1+ (Psych 4)
YAML.dump(foo)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version > 3.0 {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if !yaml_load(&call) {
            return;
        }
        let Some(selector) = call.message_loc().map(|loc| loc.span()) else { return };
        ctx.report_with_fix(
            &Self::META,
            selector,
            MSG,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(selector, b"safe_load".to_vec())],
            },
        );
    }
}

/// RuboCop's `yaml_load` node-matcher: `(send (const {nil? cbase} :YAML)
/// :load ...)` -- a bare-or-toplevel-qualified `YAML.load` call with any
/// number of arguments.
fn yaml_load(call: &CallNode<'_>) -> bool {
    if call.name().as_slice() != b"load" {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    ext::is_bare_or_toplevel_const(&receiver)
        && ext::const_name(&receiver).is_some_and(|name| name == "YAML")
}
