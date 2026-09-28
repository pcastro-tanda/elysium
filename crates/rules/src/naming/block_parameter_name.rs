//! `Naming/BlockParameterName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/block_parameter_name.rb`, sharing the
//! `UncommunicativeName` mixin checks with [`super::method_parameter_name`]
//! via [`super::uncommunicative_name`].
//!
//! # Numbered/`it` block parameters
//!
//! Upstream only defines `on_block`, never `on_numblock`, so a block using
//! `_1`/`_2` or `it` implicit parameters is never checked -- there is no
//! `node.arguments` to read. Prism mirrors that split at the node-kind
//! level: an explicit `|...|` parameter list is Prism's
//! `BlockParametersNode` (wrapping a plain `ParametersNode`), while
//! numbered and `it` parameters produce their own parameter-less
//! `NumberedParametersNode`/`ItParametersNode`. Reaching into
//! [`ruby_ast::node::ParametersNode`] only for the first two node kinds
//! reproduces the same exemption for free.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::ParametersNode;
use ruby_ast::{Node, NodeKind};

use super::uncommunicative_name::{self, NameType, UncommunicativeNameConfig};

/// Checks block parameter names for how descriptive they are.
#[derive(Debug, Clone)]
pub struct BlockParameterName {
    config: UncommunicativeNameConfig,
}

impl BlockParameterName {
    /// The block's own parameter list, unwrapping `BlockParametersNode`'s
    /// `|...|` delimiters; `None` for a parameter-less block, a brace/`do`
    /// block with no `|...|` at all, or a numbered/`it`-parameter block
    /// (see the module doc).
    fn block_parameters(parameters: Node<'_>) -> Option<ParametersNode<'_>> {
        if let Some(wrapped) = parameters.as_block_parameters_node() {
            return wrapped.parameters();
        }
        parameters.as_parameters_node()
    }
}

impl Rule for BlockParameterName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/BlockParameterName",
        department: Department::Naming,
        summary: "Checks block parameter names for how descriptive they are.",
        explanation: "\
It is highly configurable.

The `MinNameLength` config option takes an integer. It represents the
minimum amount of characters the name must be. Its default is 1. The
`AllowNamesEndingInNumbers` config option takes a boolean. When set to
false, this cop will register offenses for names ending with numbers. Its
default is true. The `AllowedNames` config option takes an array of
permitted names that will never register an offense. The `ForbiddenNames`
config option takes an array of restricted names that will always
register an offense.

```ruby
# bad
bar do |varOne, varTwo|
  varOne + varTwo
end

# With `AllowNamesEndingInNumbers` set to false
foo { |num1, num2| num1 * num2 }

# With `MinNameLength` set to number greater than 1
baz { |a, b, c| do_stuff(a, b, c) }

# good
bar do |thud, fred|
  thud + fred
end

foo { |speed, distance| speed * distance }

baz { |age, height, gender| do_stuff(age, height, gender) }
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::BlockNode],
        config: &[
            ConfigOption {
                name: "MinNameLength",
                default: ConfigDefault::Int(1),
                allowed: &[],
                doc: "Minimum number of characters a block parameter name must have.",
            },
            ConfigOption {
                name: "AllowNamesEndingInNumbers",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether a block parameter name may end with a digit.",
            },
            ConfigOption {
                name: "AllowedNames",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Block parameter names that are never flagged.",
            },
            ConfigOption {
                name: "ForbiddenNames",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Block parameter names that are always flagged.",
            },
        ],
        blind_spots: "\
A block using numbered (`_1`, `_2`) or `it` implicit parameters is never checked, matching
upstream (see the module doc).",
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
        let block = node.as_block_node().expect("kind matched");
        let Some(parameters) = block.parameters() else { return };
        let Some(params) = Self::block_parameters(parameters) else { return };
        uncommunicative_name::check(
            &self.config,
            NameType::BlockParameter,
            &params,
            ctx,
            &Self::META,
        );
    }
}
