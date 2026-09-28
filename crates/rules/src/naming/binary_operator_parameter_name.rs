//! `Naming/BinaryOperatorParameterName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/binary_operator_parameter_name.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeKind};

const EXCLUDED: &[&[u8]] = &[b"+@", b"-@", b"[]", b"[]=", b"<<", b"===", b"`", b"=~"];
const OP_LIKE_METHODS: &[&[u8]] = &[b"eql?", b"equal?"];

/// When defining binary operators, name the argument other.
#[derive(Debug, Clone)]
pub struct BinaryOperatorParameterName;

impl Rule for BinaryOperatorParameterName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/BinaryOperatorParameterName",
        department: Department::Naming,
        summary: "When defining binary operators, name the argument other.",
        explanation: "\
Makes sure that certain binary operator methods have their sole parameter
named `other`.

```ruby
# bad
def +(amount); end

# good
def +(other); end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
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
        let name = def.name().as_slice();
        if !is_op_method(name) {
            return;
        }
        let Some(params) = def.parameters() else {
            return;
        };
        if params.requireds().len() != 1
            || !params.optionals().is_empty()
            || params.rest().is_some()
            || !params.posts().is_empty()
            || !params.keywords().is_empty()
            || params.keyword_rest().is_some()
            || params.block().is_some()
        {
            return;
        }
        let Some(arg) =
            params.requireds().iter().next().and_then(|n| n.as_required_parameter_node())
        else {
            return;
        };
        let arg_name = arg.name().as_slice();
        if arg_name == b"other" || arg_name == b"_other" {
            return;
        }

        let name_str = String::from_utf8_lossy(name);
        let message =
            format!("When defining the `{name_str}` operator, name its argument `other`.");
        let arg_span = arg.location().span();

        let mut edits = vec![Edit::replace(arg_span, b"other".to_vec())];
        each_descendant(node, &mut |descendant| {
            if let Some(read) = descendant.as_local_variable_read_node() {
                if read.name().as_slice() == arg_name {
                    edits.push(Edit::replace(descendant.location().span(), b"other".to_vec()));
                }
            } else if let Some(write) = descendant.as_local_variable_write_node() {
                if write.name().as_slice() == arg_name {
                    edits.push(Edit::replace(write.name_loc().span(), b"other".to_vec()));
                }
            }
        });

        ctx.report_with_fix(
            &Self::META,
            arg_span,
            message,
            Fix { applicability: Applicability::Safe, edits },
        );
    }
}

/// Upstream's `op_method?`: is `name` a binary-operator-like method name?
fn is_op_method(name: &[u8]) -> bool {
    if EXCLUDED.contains(&name) {
        return false;
    }
    !starts_with_word_char(name) || OP_LIKE_METHODS.contains(&name)
}

/// Ruby's `/\A[[:word:]]/` against a method name: does it start with a
/// (Unicode-aware) word character?
fn starts_with_word_char(name: &[u8]) -> bool {
    match std::str::from_utf8(name).ok().and_then(|s| s.chars().next()) {
        Some(c) => c == '_' || c.is_alphanumeric(),
        None => false,
    }
}
