//! `Naming/AccessorMethodName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/accessor_method_name.rb`.
//!
//! # Arity check
//!
//! Upstream's `bad_reader_name?` is `!node.arguments?` (the `def`/`defs`
//! node's argument list, i.e. rubocop-ast's flattened
//! required/optional/rest/post/keyword/kwrest/block parameter list, is
//! empty), and `bad_writer_name?` is `node.arguments.one? &&
//! node.first_argument.arg_type?` (exactly one parameter total, and that
//! parameter is a plain required positional argument -- not an optional,
//! rest, post, keyword, kwrest, block, or `...` forwarding parameter).
//! [`ParametersNode`] exposes each of those kinds as its own list/field
//! rather than one flat ordered list, so this port sums their lengths for
//! the emptiness/one-ness check and additionally requires the sole
//! parameter (when there is exactly one) to have come from `requireds` --
//! `requireds` holds Prism's `RequiredParameterNode`, the direct analogue
//! of whitequark's `arg_type?` node.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

/// RuboCop's `MSG_READER`.
const MSG_READER: &str = "Do not prefix reader method names with `get_`.";
/// RuboCop's `MSG_WRITER`.
const MSG_WRITER: &str = "Do not prefix writer method names with `set_`.";

/// Checks for accessor methods, reader methods that begin with `get_`, and
/// writer methods that begin with `set_`. Applies to both instance and
/// class methods.
#[derive(Debug, Clone, Default)]
pub struct AccessorMethodName;

impl AccessorMethodName {
    /// RuboCop's `proper_attribute_name?`.
    fn proper_attribute_name(name: &[u8]) -> bool {
        !(name.ends_with(b"!") || name.ends_with(b"?") || name.ends_with(b"="))
    }

    /// The total number of parameters across every parameter kind, mirroring
    /// rubocop-ast's flattened `DefNode#arguments`, plus whether that total
    /// is exactly one plain required parameter (upstream's `first_argument
    /// .arg_type?` for the one-argument case).
    fn parameter_shape(def: &ruby_ast::node::DefNode<'_>) -> (usize, bool) {
        let Some(params) = def.parameters() else { return (0, false) };
        let required_count = params.requireds().len();
        let total = required_count
            + params.optionals().len()
            + usize::from(params.rest().is_some())
            + params.posts().len()
            + params.keywords().len()
            + usize::from(params.keyword_rest().is_some())
            + usize::from(params.block().is_some());
        (total, total == 1 && required_count == 1)
    }

    /// RuboCop's `bad_reader_name?`.
    fn bad_reader_name(name: &[u8], total_arguments: usize) -> bool {
        name.starts_with(b"get_") && total_arguments == 0
    }

    /// RuboCop's `bad_writer_name?`.
    fn bad_writer_name(name: &[u8], total_arguments: usize, sole_is_required_arg: bool) -> bool {
        name.starts_with(b"set_") && total_arguments == 1 && sole_is_required_arg
    }
}

impl Rule for AccessorMethodName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/AccessorMethodName",
        department: Department::Naming,
        summary: "Avoid prefixing accessor method names with `get_` or `set_`.",
        explanation: "\
Applies to both instance and class methods.

NOTE: Method names starting with `get_` or `set_` only register an offense
when the methods match the expected arity for getters and setters
respectively. Getters (`get_attribute`) must have no arguments to be
registered, and setters (`set_attribute(value)`) must have exactly one.

```ruby
# bad
def set_attribute(value)
end

# good
def attribute=(value)
end

# bad
def get_attribute
end

# good
def attribute
end

# accepted, incorrect arity for getter
def get_value(attr)
end

# accepted, incorrect arity for setter
def set_value
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let def = node.as_def_node().expect("kind matched");
        let name = def.name();
        let name = name.as_slice();
        if !Self::proper_attribute_name(name) {
            return;
        }
        let (total_arguments, sole_is_required_arg) = Self::parameter_shape(&def);
        let message = if Self::bad_reader_name(name, total_arguments) {
            MSG_READER
        } else if Self::bad_writer_name(name, total_arguments, sole_is_required_arg) {
            MSG_WRITER
        } else {
            return;
        };
        ctx.report(&Self::META, def.name_loc().span(), message);
    }
}
