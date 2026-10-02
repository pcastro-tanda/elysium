//! `Sorbet/ForbidSuperclassConstLiteral`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_superclass_const_literal.rb`.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const MSG: &str = "Superclasses must only contain constant literals";

/// Forbid superclasses which are non-literal constants.
#[derive(Debug, Clone)]
pub struct ForbidSuperclassConstLiteral;

impl Rule for ForbidSuperclassConstLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidSuperclassConstLiteral",
        department: Department::Sorbet,
        summary: "Forbid superclasses which are non-literal constants.",
        explanation: "Correct superclass `send` expressions by constant literals.\n\nSorbet, \
                      the static checker, is not (yet) able to support constructs on the \
                      following form:\n\n```ruby\nclass Foo < send_expr; end\n```\n\nMultiple \
                      occurences of this can be found in Shopify's code base like:\n\n\
                      ```ruby\nclass ShopScope < Component::TrustedIdScope[ShopIdentity::ShopId]\n\
                      ```\nor\n```ruby\nclass ApiClientEligibility < Struct.new(:api_client, \
                      :match_results, :shop)\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(class) = node.as_class_node() else { return };
        let Some(superclass) = class.superclass() else { return };
        // `(class (const ...) $(send ...) ...)`: a plain `send` -- neither
        // `csend` (`&.`) nor a call carrying a literal block, which
        // whitequark wraps in a `block` node.
        let Some(call) = superclass.as_call_node() else { return };
        if call.is_safe_navigation()
            || call.block().is_some_and(|block| block.as_block_node().is_some())
        {
            return;
        }
        ctx.report(&Self::META, superclass.span(), MSG);
    }
}
