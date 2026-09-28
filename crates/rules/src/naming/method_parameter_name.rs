//! `Naming/MethodParameterName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/method_parameter_name.rb`, sharing the
//! `UncommunicativeName` mixin checks with [`super::block_parameter_name`]
//! via [`super::uncommunicative_name`].

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeKind};

use super::uncommunicative_name::{self, NameType, UncommunicativeNameConfig};

/// Checks method parameter names for how descriptive they are.
#[derive(Debug, Clone)]
pub struct MethodParameterName {
    config: UncommunicativeNameConfig,
}

impl Rule for MethodParameterName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/MethodParameterName",
        department: Department::Naming,
        summary: "Checks method parameter names for how descriptive they are.",
        explanation: "\
It is highly configurable.

The `MinNameLength` config option takes an integer. It represents the
minimum amount of characters the name must be. Its default is 3. The
`AllowNamesEndingInNumbers` config option takes a boolean. When set to
false, this cop will register offenses for names ending with numbers. Its
default is true. The `AllowedNames` config option takes an array of
permitted names that will never register an offense. The `ForbiddenNames`
config option takes an array of restricted names that will always
register an offense.

```ruby
# bad
def bar(varOne, varTwo)
  varOne + varTwo
end

# With `AllowNamesEndingInNumbers` set to false
def foo(num1, num2)
  num1 * num2
end

# With `MinNameLength` set to number greater than 1
def baz(a, b, c)
  do_stuff(a, b, c)
end

# good
def bar(thud, fred)
  thud + fred
end

def foo(speed, distance)
  speed * distance
end

def baz(age_a, height_b, gender_c)
  do_stuff(age_a, height_b, gender_c)
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::DefNode],
        config: &[
            ConfigOption {
                name: "MinNameLength",
                default: ConfigDefault::Int(3),
                allowed: &[],
                doc: "Minimum number of characters a method parameter name must have.",
            },
            ConfigOption {
                name: "AllowNamesEndingInNumbers",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether a method parameter name may end with a digit.",
            },
            ConfigOption {
                name: "AllowedNames",
                default: ConfigDefault::StrList(&[
                    "as", "at", "by", "cc", "db", "id", "if", "in", "io", "ip", "of", "on", "os",
                    "pp", "to",
                ]),
                allowed: &[],
                doc: "Method parameter names that are never flagged.",
            },
            ConfigOption {
                name: "ForbiddenNames",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method parameter names that are always flagged.",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            config: UncommunicativeNameConfig {
                min_length: options.int("MinNameLength"),
                allow_names_ending_in_numbers: options.bool("AllowNamesEndingInNumbers"),
                allowed_names: options.str_list("AllowedNames"),
                forbidden_names: options.str_list("ForbiddenNames"),
            },
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        let Some(params) = def.parameters() else { return };
        uncommunicative_name::check(
            &self.config,
            NameType::MethodParameter,
            &params,
            ctx,
            &Self::META,
        );
    }
}
