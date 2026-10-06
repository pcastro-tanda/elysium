//! `Rails/BelongsTo`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/belongs_to.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

const SUPERFLOUS_REQUIRE_FALSE_MSG: &str = "You specified `required: false`, in Rails > 5.0 the \
     required option is deprecated and you want to use `optional: true`.";

const SUPERFLOUS_REQUIRE_TRUE_MSG: &str = "You specified `required: true`, in Rails > 5.0 the \
     required option is deprecated and you want to use `optional: false`. In most \
     configurations, this is the default and you can omit this option altogether";

/// `minimum_target_rails_version 5.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.0;

/// Use `optional: true` instead of `required: false` for `belongs_to` relations.
#[derive(Debug, Clone)]
pub struct BelongsTo {
    /// RuboCop does not run the cop below `minimum_target_rails_version`.
    supported: bool,
}

impl Rule for BelongsTo {
    const META: RuleMeta = RuleMeta {
        name: "Rails/BelongsTo",
        department: Department::Rails,
        summary: "Use `optional: true` instead of `required: false` for `belongs_to` relations.",
        explanation: "Looks for `belongs_to` associations where we control whether the \
                      association is required via the deprecated `required` option instead.\n\n\
                      Since Rails 5, `belongs_to` associations are required by default and this \
                      can be controlled through the use of `optional: true`.\n\n`required: \
                      false` is corrected to `optional: true`; `required: true` is inverted to \
                      `optional: false`, which the user may then remove depending on their \
                      defaults.\n\n```ruby\n# bad\nbelongs_to :blog, required: false\n\n# good\n\
                      belongs_to :blog, optional: true\n\n# bad\nbelongs_to :blog, required: \
                      true\n\n# good\nbelongs_to :blog, optional: false\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.target_rails_version() >= MINIMUM_TARGET_RAILS_VERSION })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"belongs_to" || call.is_safe_navigation() {
            return;
        }
        // `(send _ :belongs_to ... (hash ...))`: the last argument is a hash
        // (a `&blk` argument would be the last one in whitequark).
        if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let Some(last) = arguments.arguments().iter().last() else { return };
        let elements = match (last.as_hash_node(), last.as_keyword_hash_node()) {
            (Some(hash), _) => hash.elements(),
            (None, Some(hash)) => hash.elements(),
            _ => return,
        };
        // `<$(pair (sym :required) $boolean) ...>`: the first such pair.
        let found = elements.iter().find_map(|element| {
            let pair = element.as_assoc_node()?;
            if pair.key().as_symbol_node()?.unescaped() != b"required" {
                return None;
            }
            let value = pair.value();
            let boolean = value.as_true_node().is_some() || value.as_false_node().is_some();
            boolean.then(|| (pair.as_node().span(), value.as_true_node().is_some()))
        });
        let Some((pair_span, is_true)) = found else { return };
        let (message, replacement) = if is_true {
            (SUPERFLOUS_REQUIRE_TRUE_MSG, "optional: false")
        } else {
            (SUPERFLOUS_REQUIRE_FALSE_MSG, "optional: true")
        };
        let Some(selector) = call.message_loc() else { return };
        let selector = ruby_ast::LocationExt::span(&selector);
        ctx.report_with_fix(
            &Self::META,
            selector,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(pair_span, replacement.as_bytes().to_vec())],
            },
        );
    }
}
