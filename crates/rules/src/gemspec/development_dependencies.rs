//! `Gemspec/DevelopmentDependencies`, ported from RuboCop's
//! `lib/rubocop/cop/gemspec/development_dependencies.rb`.
//!
//! `AllowedGems` -- a string list, possibly absent or explicitly `~` (nil)
//! in YAML, in which case RuboCop's `Array(cop_config['AllowedGems'])`
//! coerces to an empty array; `options.str_list` already returns an empty
//! `Vec` for either case, reproducing that coercion.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Gemfile,
    GemsRb,
    Gemspec,
}

impl Style {
    fn label(self) -> &'static str {
        match self {
            Style::Gemfile => "Gemfile",
            Style::GemsRb => "gems.rb",
            Style::Gemspec => "gemspec",
        }
    }
}

/// Enforce that development dependencies for a gem are specified in `Gemfile`, rather than in
/// the `gemspec` using `add_development_dependency`.
#[derive(Debug, Clone)]
pub struct DevelopmentDependencies {
    style: Style,
    allowed_gems: Vec<String>,
}

impl Rule for DevelopmentDependencies {
    const META: RuleMeta = RuleMeta {
        name: "Gemspec/DevelopmentDependencies",
        department: Department::Gemspec,
        summary: "Specify development dependencies in Gemfile.",
        explanation: "\
Enforce that development dependencies for a gem are specified in
`Gemfile`, rather than in the `gemspec` using
`add_development_dependency`. Alternatively, using `EnforcedStyle:
gemspec`, enforce that all dependencies are specified in `gemspec`,
rather than in `Gemfile`.

```ruby
# EnforcedStyle: Gemfile (default)
# Specify runtime dependencies in your gemspec,
# but all other dependencies in your Gemfile.

# bad
# example.gemspec
s.add_development_dependency \"foo\"

# good
# Gemfile
gem \"foo\"

# good
# gems.rb
gem \"foo\"
```

```ruby
# EnforcedStyle: gemspec
# Specify all dependencies in your gemspec.

# bad
# Gemfile
gem \"foo\"

# good
# example.gemspec
s.add_development_dependency \"foo\"
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[
            linter::ConfigOption {
                name: "EnforcedStyle",
                default: linter::ConfigDefault::Str("Gemfile"),
                allowed: &["Gemfile", "gems.rb", "gemspec"],
                doc: "The dependency file development dependencies are expected in.",
            },
            linter::ConfigOption {
                name: "AllowedGems",
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Gems that can be specified as a development dependency in either file.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "gems.rb" => Style::GemsRb,
            "gemspec" => Style::Gemspec,
            _ => Style::Gemfile,
        };
        Ok(Self { style, allowed_gems: options.str_list("AllowedGems") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let name = call.name().as_slice();

        let is_forbidden = |gem_name: &[u8]| -> bool {
            !self.allowed_gems.iter().any(|allowed| allowed.as_bytes() == gem_name)
        };

        let matched = match self.style {
            Style::Gemfile | Style::GemsRb => {
                if name != b"add_development_dependency" {
                    return;
                }
                first_str_arg(&call).is_some_and(|arg| is_forbidden(&arg))
            }
            Style::Gemspec => {
                if name != b"gem" {
                    return;
                }
                first_str_arg(&call).is_some_and(|arg| is_forbidden(&arg))
            }
        };

        if !matched {
            return;
        }
        let message = format!("Specify development dependencies in {}.", self.style.label());
        ctx.report(&Self::META, node.span(), message);
    }
}

/// The first argument's unescaped string value, when it is a plain (non-interpolated) string
/// literal -- RuboCop's `(str #forbidden_gem? ...)` pattern argument.
fn first_str_arg(call: &ruby_ast::node::CallNode<'_>) -> Option<Vec<u8>> {
    let args = call.arguments()?;
    let first = args.arguments().iter().next()?;
    let string = first.as_string_node()?;
    Some(string.unescaped().to_vec())
}
