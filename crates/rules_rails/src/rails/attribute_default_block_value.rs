//! `Rails/AttributeDefaultBlockValue`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/attribute_default_block_value.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Pass method in a block to `:default` option.";

/// Looks for `attribute` class methods that specify a `:default` option whose
/// value is an array, hash or method call without a block.
#[derive(Debug, Clone)]
pub struct AttributeDefaultBlockValue;

impl Rule for AttributeDefaultBlockValue {
    const META: RuleMeta = RuleMeta {
        name: "Rails/AttributeDefaultBlockValue",
        department: Department::Rails,
        summary: "Pass method call in block for attribute option `default`.",
        explanation: "Looks for `attribute` class methods that specify a `:default` option \
                      which value is an array, string literal or method call without a block. \
                      It will accept all other values, such as string, symbol, integer and \
                      float literals as well as constants.\n\n```ruby\n# bad\nattribute \
                      :confirmed_at, :datetime, default: Time.zone.now\n\n# good\nattribute \
                      :confirmed_at, :datetime, default: -> { Time.zone.now }\n\n# bad\n\
                      attribute :roles, :string, array: true, default: []\n\n# good\nattribute \
                      :roles, :string, array: true, default: -> { [] }\n\n# good\nattribute \
                      :roles, :string, array: true, default: [].freeze\n\n# bad\nattribute \
                      :configuration, default: {}\n\n# good\nattribute :configuration, \
                      default: -> { {} }\n\n# good\nattribute :role, :string, default: \
                      :customer\n```",
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
        if call.name().as_slice() != b"attribute"
            || call.receiver().is_some()
            || call.is_safe_navigation()
            || call.block().is_some_and(|block| block.as_block_argument_node().is_some())
        {
            return;
        }
        // (send nil? :attribute _ ?_ (hash <$#attribute ...>))
        let Some(arguments) = call.arguments() else { return };
        let arguments: Vec<Node<'_>> = arguments.arguments().iter().collect();
        if !(2..=3).contains(&arguments.len()) {
            return;
        }
        let Some(last) = arguments.last() else { return };
        let elements: Vec<Node<'_>> = if let Some(hash) = last.as_keyword_hash_node() {
            hash.elements().iter().collect()
        } else if let Some(hash) = last.as_hash_node() {
            hash.elements().iter().collect()
        } else {
            return;
        };
        let Some(value) = elements.iter().find_map(|element| {
            let assoc = element.as_assoc_node()?;
            let key = assoc.key();
            let symbol = key.as_symbol_node()?;
            (symbol.unescaped() == b"default").then(|| assoc.value())
        }) else {
            return;
        };

        if is_frozen_literal_default(&value) || !is_offender(&value) {
            return;
        }
        let span = value.span();
        let mut replacement = b"-> { ".to_vec();
        replacement.extend_from_slice(ctx.text(span));
        replacement.extend_from_slice(b" }");
        ctx.report_with_fix(
            &Self::META,
            span,
            MSG,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    }
}

/// `(send {(array) (hash)} :freeze)`: an empty literal frozen.
fn is_frozen_literal_default(value: &Node<'_>) -> bool {
    let Some(call) = value.as_call_node() else { return false };
    if call.name().as_slice() != b"freeze"
        || call.is_safe_navigation()
        || call.arguments().is_some()
        || call.block().is_some()
    {
        return false;
    }
    let Some(receiver) = call.receiver() else { return false };
    receiver.as_array_node().is_some_and(|array| array.elements().iter().next().is_none())
        || receiver.as_hash_node().is_some_and(|hash| hash.elements().iter().next().is_none())
}

/// `TYPE_OFFENDERS = %i[send array hash]`.
fn is_offender(value: &Node<'_>) -> bool {
    if let Some(call) = value.as_call_node() {
        // `csend` and calls with a literal block are other node types.
        return !(call.is_safe_navigation()
            || call.block().is_some_and(|block| block.as_block_node().is_some()));
    }
    value.as_array_node().is_some()
        || value.as_hash_node().is_some()
        || value.as_keyword_hash_node().is_some()
}
